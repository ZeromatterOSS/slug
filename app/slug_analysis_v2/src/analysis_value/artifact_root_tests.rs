use super::*;

#[test]
fn extracted_root_survives_producer_heap_and_retains_complete_configuration() {
    let root = test_artifact_root();
    let retained = {
        let module = starlark::environment::Module::new();
        let artifact = slug_build_api_v2::AnalysisArtifact::Derived {
            owner: AnalysisConfiguredTargetKey::new(
                slug_identity_v2::CanonicalLabel::parse("@@//:producer").unwrap(),
                b"owner".as_slice(),
            ),
            output: slug_build_api_v2::ActionOutput::new(
                root.clone(),
                "pkg/output",
                slug_build_api_v2::ActionOutputKind::File,
            ),
        };
        let file = module
            .heap()
            .alloc_simple(AnalysisArtifactValue::new(artifact));
        let extracted = file.get_attr("root", module.heap()).unwrap().unwrap();
        AnalysisValueLowerer::default()
            .lower(extracted, "$root")
            .unwrap()
    };
    let heap = FrozenHeap::new();
    let restored = AnalysisValueMaterializer::new(&heap)
        .value(&retained)
        .unwrap();
    assert_eq!(
        slug_loading_v2::artifact_root_value(restored.to_value()),
        Some(slug_build_api_v2::AnalysisArtifactRoot::Derived(
            root.clone()
        ))
    );
    let changed =
        root.configuration()
            .with_starlark_option(slug_configuration_v2::StarlarkOption::string(
                slug_identity_v2::CanonicalLabel::parse("@@//:unused").unwrap(),
                "changed",
                slug_configuration_v2::StarlarkOptionScope::Default,
            ));
    let changed = AnalysisValue::artifact_root(slug_build_api_v2::AnalysisArtifactRoot::Derived(
        slug_build_api_v2::DerivedArtifactRoot::bin(changed).unwrap(),
    ));
    assert!(!retained.publication_eq(&changed));
    assert!(retained.publication_eq(&AnalysisValue::artifact_root(
        slug_build_api_v2::AnalysisArtifactRoot::Derived(root)
    )));
}
