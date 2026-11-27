use slug_build_api_v2::DepsetOrder;
use slug_build_api_v2::ProviderId;
use slug_identity_v2::CanonicalLabel;

use super::*;

#[test]
fn budgeted_target_projects_native_providers_with_shared_fields_and_files() {
    let key = ConfiguredNodeKey::null(CanonicalLabel::parse("@@//pkg:probe").unwrap());
    let id = ProviderId::new("@@//pkg:defs.bzl", "ProbeInfo").unwrap();
    let artifact = AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:data").unwrap());
    let files = AnalysisDepset::new(
        DepsetOrder::Default,
        vec![AnalysisValue::artifact(artifact)],
        Vec::new(),
    )
    .unwrap();
    let shared = AnalysisValue::list([AnalysisValue::string("shared field")]);
    let providers = ProviderCollection::new(vec![
        ProviderValue::DefaultInfo(
            slug_build_api_v2::DefaultInfo::from_files(files.dupe()).unwrap(),
        ),
        ProviderValue::Occurrence(ProviderOccurrence::new(
            ProviderIdentity::user(id.dupe()),
            [
                ("first", shared.dupe()),
                ("second", shared),
                ("files", AnalysisValue::depset(files)),
            ],
        )),
    ])
    .unwrap();

    let heap = FrozenHeap::new();
    let budget = ExecutionBudget::new(50_000, 1 << 20, 32);
    let result = AnalysisValueMaterializer::new_with_budget(&heap, Some(&budget))
        .budgeted_cquery_target(&key, &providers)
        .unwrap();
    let target = AnalysisConfiguredTargetValue::from_value(result.to_value()).unwrap();
    assert_eq!(target.retained.identity().label(), key.label());
    let probe = target
        .providers
        .get(&ProviderIdentity::user(id))
        .unwrap()
        .to_value();
    let (_, fields) = starlark_user_provider_fields(probe).unwrap();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0].0.as_str(), "files");
    assert!(fields[0].1.ptr_eq(target.files().unwrap().to_value()));
    assert!(fields[1].1.ptr_eq(fields[2].1));
    assert_eq!(budget.check(), Ok(()));

    let short_heap = FrozenHeap::new();
    let short = ExecutionBudget::new(50_000, 1, 32);
    let error = AnalysisValueMaterializer::new_with_budget(&short_heap, Some(&short))
        .budgeted_cquery_target(&key, &providers)
        .unwrap_err();
    assert!(matches!(error.kind(), starlark::ErrorKind::Other(cause)
        if cause.downcast_ref::<ExecutionLimit>() == Some(&ExecutionLimit::AllocationBytes)));
    assert_eq!(short_heap.allocated_bytes(), 0);
}

#[test]
fn budgeted_target_nesting_refusal_is_typed_and_sticky() {
    let mut value = AnalysisValue::string("leaf");
    for _ in 0..12 {
        value = AnalysisValue::tuple([value]);
    }
    let heap = FrozenHeap::new();
    let budget = ExecutionBudget::new(10_000, 1 << 20, 8);
    let mut projection = AnalysisValueMaterializer::new_with_budget(&heap, Some(&budget));
    let error = projection.project_value(&value).unwrap_err();
    assert!(matches!(error.kind(), starlark::ErrorKind::Other(cause)
        if cause.downcast_ref::<ExecutionLimit>() == Some(&ExecutionLimit::StructuralDepth)));
    assert_eq!(budget.check(), Err(ExecutionLimit::StructuralDepth));
    assert_eq!(heap.allocated_bytes(), 0);
}
