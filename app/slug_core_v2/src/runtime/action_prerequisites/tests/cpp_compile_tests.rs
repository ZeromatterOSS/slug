use super::*;

#[path = "../../../../../slug_build_api_v2/tests/cpp_compile_support/mod.rs"]
mod fixture;

#[test]
fn cpp_compile_prerequisites_and_source_staging_refuse_before_content_observation() {
    let owner = key("compile", false);
    let root = slug_build_api_v2::DerivedArtifactRoot::bin(
        owner.configuration().slug_configuration().unwrap().clone(),
    )
    .unwrap();
    let action = fixture::action(root.clone());
    assert!(action.cpp_compile_spec().is_some());
    let compile_output = AnalysisArtifact::Derived {
        owner: owner.artifact_owner(),
        output: action.outputs()[0].clone(),
    };
    let consumer = spawn(
        vec![compile_output.clone()],
        vec![ActionOutput::new(root, "consumer", ActionOutputKind::File)],
    );
    let closure =
        ValidatedActionClosure::new(Arc::from([node(owner.clone(), vec![action, consumer])]))
            .unwrap();
    for ordinal in [0, 1] {
        assert_eq!(
            ActionPrerequisitePlan::new(&closure, &owner, ordinal)
                .unwrap_err()
                .as_ref(),
            "CppCompile discovery-aware prerequisites are not admitted"
        );
    }
    assert_eq!(
        super::super::plan_roots(
            &closure,
            super::super::PlanRoots::Artifacts(&[compile_output])
        )
        .unwrap_err()
        .as_ref(),
        "CppCompile discovery-aware prerequisites are not admitted"
    );
    let selected = &closure.owners()[0].actions()[0];
    assert_eq!(
        crate::runtime::dice::source_staging::source_spawn(selected)
            .unwrap_err()
            .as_ref(),
        "CppCompile discovery-aware source staging is not admitted"
    );
    // A supported typed Spawn still passes this family gate; its later declared
    // source validation is not bypassed or exercised by this refusal test.
    assert!(
        crate::runtime::dice::source_staging::source_spawn(&closure.owners()[0].actions()[1])
            .is_ok()
    );
}
