//! Private source helper selected from its unchanged, fully loaded module.
use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::AnalysisArtifact;
use slug_build_api_v2::AnalysisConfiguredTargetKey;

use super::*;
use crate::subrule_invocation::AnalysisArtifactValue;

#[test]
fn unchanged_repo_relative_path_uses_genuine_source_generated_and_shared_solib_roots() {
    let helper = eval_bzl_with_loaded_children(
        RULES_CC_COMPILATION_HELPER_SOURCE,
        BzlModuleIdentity {
            label: CanonicalLabel::parse(
                "@@rules_cc+//cc/private/compile:cc_compilation_helper.bzl",
            )
            .unwrap(),
            source: slug_build_api_v2::ModuleSourceOrigin::Workspace(PathBuf::from(
                "/rules_cc/cc/private/compile/cc_compilation_helper.bzl",
            )),
            repository_mapping: Arc::from([(
                ApparentRepoName::new("bazel_skylib").unwrap(),
                CanonicalRepoName::new("bazel_skylib+").unwrap(),
            )]),
        },
        &complete_rules_cc_compilation_helper_children(),
    )
    .unwrap();
    let function = helper.get_any_visibility("_repo_relative_path").unwrap().0;
    let root = crate::artifact_root::test_root();
    let owner = AnalysisConfiguredTargetKey::new(
        CanonicalLabel::parse("@@rules_cc+//pkg:owner").unwrap(),
        b"action-owner".as_slice(),
    );
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    for (artifact, expected) in [
        (
            AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:source.h").unwrap()),
            "pkg/source.h",
        ),
        (
            AnalysisArtifact::Source(CanonicalLabel::parse("@@rules_cc+//pkg:source.h").unwrap()),
            "pkg/source.h",
        ),
        (
            AnalysisArtifact::Derived {
                owner: owner.clone(),
                output: ActionOutput::new(
                    root.clone(),
                    "external/rules_cc+/pkg/generated.h",
                    ActionOutputKind::File,
                ),
            },
            "pkg/generated.h",
        ),
        (
            AnalysisArtifact::Derived {
                owner,
                output: ActionOutput::new(
                    root.clone(),
                    "_solib_k8/libshared.so",
                    ActionOutputKind::File,
                ),
            },
            "_solib_k8/libshared.so",
        ),
    ] {
        let file = module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(artifact));
        let result = eval.eval_function(function.value(), &[file], &[]).unwrap();
        assert_eq!(result.unpack_str(), Some(expected));
    }
}
