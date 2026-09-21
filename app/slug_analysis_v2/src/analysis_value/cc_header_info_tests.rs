use slug_build_api_v2::ActionOutput;
use slug_build_api_v2::ActionOutputKind;
use slug_build_api_v2::CcHeaderInfo;
use slug_build_api_v2::CcHeaderInfoInput;
use slug_identity_v2::CanonicalLabel;

use super::*;

#[test]
fn header_info_owns_artifacts_and_hidden_edges_after_producer_heap_disposal() {
    let source = AnalysisArtifact::Source(CanonicalLabel::parse("@@//:header.h").unwrap());
    let derived = AnalysisArtifact::Derived {
        owner: slug_build_api_v2::AnalysisConfiguredTargetKey::new(
            CanonicalLabel::parse("@@//:producer").unwrap(),
            vec![2, 3],
        ),
        output: ActionOutput::new(
            crate::analysis_value::test_artifact_root(),
            "header.pcm",
            ActionOutputKind::File,
        ),
    };
    let child = CcHeaderInfo::create(
        CcHeaderInfoInput {
            header_module: Some(derived.clone()),
            modular_public_headers: vec![source.clone()].into(),
            ..Default::default()
        },
        false,
        &[],
    )
    .unwrap();
    let parent = CcHeaderInfo::create(
        CcHeaderInfoInput {
            header_module: Some(derived.clone()),
            pic_header_module: Some(derived.clone()),
            separate_module: Some(derived.clone()),
            separate_pic_module: Some(derived.clone()),
            modular_public_headers: vec![source.clone()].into(),
            modular_private_headers: vec![source.clone()].into(),
            textual_headers: vec![source.clone()].into(),
            separate_module_headers: vec![source.clone(), source.clone()].into(),
            deps: vec![child.clone(), child.clone()].into(),
        },
        false,
        &[],
    )
    .unwrap();
    let retained = {
        let heap = FrozenHeap::new();
        let parent = slug_loading_v2::alloc_frozen_cc_header_info(&heap, parent);
        let child = slug_loading_v2::alloc_frozen_cc_header_info(&heap, child);
        let value = heap.alloc(AllocTuple([parent, parent, child]));
        AnalysisValueLowerer::default()
            .lower(value.to_value(), "header")
            .unwrap()
    };
    let target = FrozenHeap::new();
    let restored = AnalysisValueMaterializer::new(&target)
        .value(&retained)
        .unwrap();
    let values = TupleRef::from_value(restored.to_value())
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert!(values[0].equals(values[1]).unwrap());
    assert!(!values[0].equals(values[2]).unwrap());
    let info = slug_loading_v2::cc_header_info(values[0]).unwrap();
    let child = slug_loading_v2::cc_header_info(values[2]).unwrap();
    assert_eq!(info.model().deps.as_ref(), &[child.clone(), child]);
    let scratch = starlark::environment::Module::new();
    for name in [
        "header_module",
        "pic_header_module",
        "separate_module",
        "separate_pic_module",
    ] {
        let value = values[0].get_attr(name, scratch.heap()).unwrap().unwrap();
        assert_eq!(
            AnalysisValueLowerer::default().lower(value, name).unwrap(),
            AnalysisValue::artifact(derived.clone())
        );
    }
    for (name, count) in [
        ("modular_public_headers", 1),
        ("modular_private_headers", 1),
        ("textual_headers", 1),
        ("separate_module_headers", 2),
    ] {
        let value = values[0].get_attr(name, scratch.heap()).unwrap().unwrap();
        let list = ListRef::from_value(value).unwrap();
        assert_eq!(list.len(), count);
        for value in list.iter() {
            assert_eq!(
                AnalysisValueLowerer::default().lower(value, name).unwrap(),
                AnalysisValue::artifact(source.clone())
            );
        }
    }
    assert!(
        values[0]
            .get_attr("deps", scratch.heap())
            .unwrap()
            .is_none()
    );
    let twice = AnalysisValueLowerer::default()
        .lower(restored.to_value(), "again")
        .unwrap();
    assert_eq!(retained, twice);
    assert!(retained.publication_eq(&twice));
}
