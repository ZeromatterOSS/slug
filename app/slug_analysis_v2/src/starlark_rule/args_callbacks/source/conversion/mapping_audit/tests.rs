use super::*;

fn mapping(entries: usize) -> Mapping {
    (0..entries)
        .map(|n| {
            (
                slug_identity_v2::ApparentRepoName::new(format!("apparent{n}")).unwrap(),
                slug_identity_v2::CanonicalRepoName::new(format!("canonical{n}+")).unwrap(),
            )
        })
        .collect::<Vec<_>>()
        .into()
}
fn manifest(mapping: &Mapping) -> BzlLoadManifest {
    let mut value = super::super::super::tests::manifest("@@root//:source.bzl", [1; 32]);
    value.root.repository_mapping = mapping.clone();
    value.direct_children = Arc::from([value.root.clone()]);
    value.reachable = Arc::from([slug_loading_v2::BzlModuleSourceProvenance::new(
        value.root.clone(),
        [1; 32],
    )]);
    value
}
fn convert_cost(
    value: &BzlLoadManifest,
    audits: &mut MappingAudits,
    builder: &mut PublishedValueArenaBuilder,
) -> (CallableSourceAuthority, usize) {
    let before = builder.charged_visits();
    let result = convert(value, audits, builder, &mut 65536).unwrap();
    (result, builder.charged_visits() - before)
}

#[test]
fn actual_mapping_guards_reuse_complete_bytes_and_release_with_the_session() {
    let shared = mapping(8);
    let weak = Arc::downgrade(&shared);
    let mut audits = MappingAudits::default();
    let mut builder = PublishedValueArenaBuilder::new();
    {
        let value = manifest(&shared);
        let (first, first_work) = convert_cost(&value, &mut audits, &mut builder);
        let (again, again_work) = convert_cost(&value, &mut audits, &mut builder);
        // Host mapping scans remain unchanged (21 first, 8 repeated). Each
        // constructor additionally pays 3+4*3+8+1=24 real native operations:
        // two identity passes, actual mapping scan, probe/guard and insertion.
        assert_eq!(first_work, 45);
        assert_eq!(again_work, 32);
        assert_eq!(first, again);
        assert_eq!(audits.committed.len(), 1);
        let separate: Mapping = shared.to_vec().into();
        assert!(!Arc::ptr_eq(&shared, &separate));
        let different_allocation = manifest(&separate);
        let (equal, work) = convert_cost(&different_allocation, &mut audits, &mut builder);
        assert_eq!(work, first_work);
        assert_eq!(first, equal);
        assert_eq!(audits.committed.len(), 2);
        let mut fresh = MappingAudits::default();
        let mut fresh_builder = PublishedValueArenaBuilder::new();
        assert_eq!(
            convert_cost(&value, &mut fresh, &mut fresh_builder).1,
            first_work
        );
        let plan = forecast(&value, &audits, &mut builder, &mut 65536).unwrap();
        let independent =
            forecast(&different_allocation, &audits, &mut builder, &mut 65536).unwrap();
        assert_eq!(plan.output_bytes, independent.output_bytes);
        let before_bytes = plan.output_bytes;
        drop((plan, independent));
        let mut pairs = shared.to_vec();
        let previous = pairs[7].1.as_str().len();
        pairs[7].1 =
            slug_identity_v2::CanonicalRepoName::new("changed_final_canonical_repository+")
                .unwrap();
        let delta = pairs[7].1.as_str().len() - previous;
        let changed = manifest(&pairs.into());
        let changed_plan = forecast(&changed, &audits, &mut builder, &mut 65536).unwrap();
        assert_eq!(changed_plan.output_bytes - before_bytes, 3 * delta);
    }
    // Producer handles may disappear; the owning guard prevents address ABA.
    drop(shared);
    assert!(weak.upgrade().is_some());
    drop(audits);
    assert!(
        weak.upgrade().is_some(),
        "paid authority guards remain builder-owned"
    );
    drop(builder);
    assert!(weak.upgrade().is_none());
}

#[test]
fn mapping_growth_and_commit_admission_refuse_before_guard_clones() {
    let shared = mapping(2);
    let value = manifest(&shared);
    let references = Arc::strong_count(&shared);
    let mut probe = PublishedValueArenaBuilder::new();
    let plan = forecast(&value, &MappingAudits::default(), &mut probe, &mut 65536).unwrap();
    let forecast_work = probe.charged_visits();
    drop(plan);
    let mut reference = PublishedValueArenaBuilder::new();
    drop(convert_cost(
        &value,
        &mut MappingAudits::default(),
        &mut reference,
    ));
    let full_bytes = reference.charged_bytes();
    drop(reference);
    for (bytes, remaining) in [
        (peak::<BorrowedProof<'_>>(1).unwrap() - 1, 65536),
        (full_bytes - 1, 65536),
        (full_bytes, forecast_work),
    ] {
        let mut audits = MappingAudits::default();
        let mut builder = PublishedValueArenaBuilder::new();
        builder
            .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - bytes)
            .unwrap();
        assert!(convert(&value, &mut audits, &mut builder, &mut { remaining }).is_err());
        assert!(audits.committed.is_empty());
        assert_eq!(audits.credit, 0);
        assert_eq!(Arc::strong_count(&shared), references);
        assert!(audits.check().is_err());
        assert!(builder.charge_bytes(0).is_err());
        assert!(convert(&value, &mut audits, &mut builder, &mut 65536).is_err());
    }
    assert!(count(usize::MAX, 1).is_err());
    assert!(peak::<OwnedProof>(usize::MAX).is_err());
}

#[test]
fn later_mapping_growth_keeps_pending_state_unchanged_on_refusal() {
    let values = (0..5).map(|_| mapping(1)).collect::<Vec<_>>();
    let audits = MappingAudits::default();
    let mut builder = PublishedValueArenaBuilder::new();
    let mut plan = MappingPlan::new(&mut builder).unwrap();
    for value in &values[..4] {
        plan.mapping_bytes(value, &audits, &mut builder, &mut 65536)
            .unwrap();
    }
    let previous = plan.credit;
    assert!(peak::<BorrowedProof<'_>>(5).unwrap() > previous);
    builder
        .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - builder.charged_bytes() - 1)
        .unwrap();
    assert!(
        plan.mapping_bytes(&values[4], &audits, &mut builder, &mut 65536)
            .is_err()
    );
    assert_eq!(plan.pending.len(), 4);
    assert_eq!(plan.credit, previous);
    assert!(builder.charge_bytes(0).is_err());
    assert!(values.iter().all(|value| Arc::strong_count(value) == 1));
}

#[test]
fn later_authority_refusal_poison_keeps_committed_guards_until_drop() {
    // A small shared mapping repeated in metadata crosses the independent
    // authority fact cap after conversion admission, without a large payload.
    let shared = mapping(256);
    let weak = Arc::downgrade(&shared);
    let mut value = manifest(&shared);
    value.direct_children = vec![value.root.clone(); 256].into();
    let mut audits = MappingAudits::default();
    let mut builder = PublishedValueArenaBuilder::new();
    let error = convert(&value, &mut audits, &mut builder, &mut 65536).unwrap_err();
    assert!(error.contains("source facts"), "{error}");
    assert_eq!(audits.committed.len(), 1);
    assert!(audits.check().is_err());
    assert!(builder.charge_visit().is_err());
    // Even an existing proof cannot bypass this failed session.
    assert!(
        convert(&manifest(&shared), &mut audits, &mut builder, &mut 65536)
            .unwrap_err()
            .contains("previously failed")
    );
    drop(value);
    drop(shared);
    assert!(weak.upgrade().is_some());
    drop(audits);
    assert!(weak.upgrade().is_none());
}

#[test]
fn all_hit_conversions_admit_the_complete_fixed_plan_before_output_clones() {
    let shared = mapping(2);
    let value = manifest(&shared);
    let initial_references = Arc::strong_count(&shared);
    let mut reference = PublishedValueArenaBuilder::new();
    let mut reference_audits = MappingAudits::default();
    drop(convert_cost(&value, &mut reference_audits, &mut reference));
    let before = reference.charged_bytes();
    drop(convert_cost(&value, &mut reference_audits, &mut reference));
    let full = reference.charged_bytes() - before;
    drop((reference, reference_audits));
    for succeeds in [false, true] {
        let mut audits = MappingAudits::default();
        let mut builder = PublishedValueArenaBuilder::new();
        drop(convert_cost(&value, &mut audits, &mut builder));
        assert!(audits.header_admitted);
        let plan = forecast(&value, &audits, &mut builder, &mut 65536).unwrap();
        assert!(plan.pending.is_empty());
        assert!(full >= plan.output_bytes + size_of::<MappingPlan<'_>>());
        drop(plan);
        let allowance = full - usize::from(!succeeds);
        builder
            .charge_bytes(
                slug_build_api_v2::published_value::BYTE_LIMIT
                    - builder.charged_bytes()
                    - allowance,
            )
            .unwrap();
        let references = Arc::strong_count(&shared);
        let result = convert(&value, &mut audits, &mut builder, &mut 65536);
        assert_eq!(result.is_ok(), succeeds);
        drop(result);
        assert_eq!(
            Arc::strong_count(&shared),
            references + if succeeds { 3 } else { 0 }
        );
        assert_eq!(audits.committed.len(), 1);
        if succeeds {
            assert_eq!(
                builder.charged_bytes(),
                slug_build_api_v2::published_value::BYTE_LIMIT
            );
        } else {
            assert!(audits.check().is_err());
            assert!(builder.charge_bytes(0).is_err());
        }
        drop(builder);
        assert_eq!(Arc::strong_count(&shared), initial_references + 1);
        drop(audits);
        assert_eq!(Arc::strong_count(&shared), initial_references);
    }
}

fn seal_source(
    mut builder: PublishedValueArenaBuilder,
    authority: CallableSourceAuthority,
) -> slug_build_api_v2::published_value::PublishedValueArena {
    use slug_build_api_v2::published_value::PublishedCode;
    use slug_build_api_v2::published_value::PublishedNode;
    let node = builder.reserve().unwrap();
    let dependencies = builder.append_edges(&[]).unwrap();
    builder
        .set_node(
            node,
            PublishedNode::Code(PublishedCode {
                source: authority,
                name: "source".into(),
                definition: slug_build_api_v2::CallableDefinitionSpan { start: 0, end: 1 },
                dependencies,
                globals: Arc::from([]),
            }),
        )
        .unwrap();
    let module = starlark::environment::Module::new();
    module.set_extra_value(
        module
            .heap()
            .alloc(starlark::values::tuple::AllocTuple(Vec::<
                starlark::values::Value<'_>,
            >::new())),
    );
    let frozen = module.freeze().unwrap();
    let originals = crate::analysis_value::published::StarlarkPublishedOriginals::new(
        frozen.owned_extra_value().unwrap(),
        builder.imports().to_vec().into(),
    )
    .unwrap();
    builder.seal(Arc::new(originals)).unwrap()
}

#[test]
fn mapping_split_keeps_complete_output_and_paid_total_after_scratch_is_dropped() {
    let shared = mapping(8);
    let value = manifest(&shared);
    let mut forecast_builder = PublishedValueArenaBuilder::new();
    let plan = forecast(
        &value,
        &MappingAudits::default(),
        &mut forecast_builder,
        &mut 65536,
    )
    .unwrap();
    let output = plan.output_bytes;
    let scratch = peak::<OwnedProof>(plan.pending.len()).unwrap()
        + plan.pending.len() * size_of::<OwnedProof>();
    let before = forecast_builder.charged_bytes();
    let mut reference_admission = PublishedValueArenaBuilder::new();
    reference_admission
        .begin_source_authority(
            output
                - output_transfer_bytes(value.direct_children.len(), value.reachable.len())
                    .unwrap(),
            output_transfer_bytes(value.direct_children.len(), value.reachable.len()).unwrap(),
            scratch,
            identity_bound(SourceManifest::Raw(&value)).unwrap(),
        )
        .unwrap()
        .abort();
    let full_admission = reference_admission.charged_bytes();
    let (_, preparation) = plan
        .admit(
            &MappingAudits::default(),
            &mut forecast_builder,
            &mut 65536,
            output_transfer_bytes(value.direct_children.len(), value.reachable.len()).unwrap(),
            identity_bound(SourceManifest::Raw(&value)).unwrap(),
        )
        .unwrap();
    preparation.abort();
    assert_eq!(forecast_builder.charged_bytes() - before, full_admission);
    assert!(full_admission >= output + scratch);
    drop(plan);
    let mut builder = PublishedValueArenaBuilder::new();
    let mut audits = MappingAudits::default();
    let (authority, _) = convert_cost(&value, &mut audits, &mut builder);
    drop(audits);
    let native = super::super::tests::native_cost(&authority);
    let transfer = std::mem::size_of_val(authority.data().direct_children.as_ref())
        + std::mem::size_of_val(authority.data().reachable.as_ref());
    assert_eq!(output, native + transfer);
    let reference = PublishedValueArenaBuilder::new();
    let actual = seal_source(builder, authority.clone());
    let baseline = seal_source(reference, authority.clone());
    assert_eq!(
        actual.shape().bytes,
        baseline.shape().bytes,
        "complete native Q stays retained; only duplicate Q and dead transfer backing are excluded"
    );
    assert_eq!(actual.sources(), baseline.sources());
    assert_eq!(actual.sources()[0], authority);
    assert_eq!(actual.shape().visits, baseline.shape().visits);
    let mut imported_actual = PublishedValueArenaBuilder::new();
    imported_actual.import(actual.clone()).unwrap();
    let paid_import = imported_actual.charged_bytes();
    assert!(paid_import >= actual.shape().bytes);
    let mut imported_baseline = PublishedValueArenaBuilder::new();
    imported_baseline.import(baseline.clone()).unwrap();
    assert_eq!(paid_import, imported_baseline.charged_bytes());
    let resealed_actual = seal_source(imported_actual, authority.clone());
    let resealed_baseline = seal_source(imported_baseline, authority);
    let shape = |shape: slug_build_api_v2::published_value::PublicationShape| {
        (
            shape.nodes,
            shape.edges,
            shape.bytes,
            shape.visits,
            shape.depth,
        )
    };
    assert_eq!(
        shape(resealed_actual.shape()),
        shape(resealed_baseline.shape())
    );
    assert_eq!(resealed_actual.sources(), resealed_baseline.sources());
    assert_eq!(
        shape(resealed_actual.imports()[0].shape()),
        shape(actual.shape())
    );
}

#[test]
fn source_preparation_pending_work_is_native_first_and_abandonment_poisoned() {
    let shared = mapping(2);
    let value = manifest(&shared);
    for host_exhausted in [false, true] {
        let audits = MappingAudits::default();
        let mut builder = PublishedValueArenaBuilder::new();
        let plan = forecast(&value, &audits, &mut builder, &mut 65536).unwrap();
        assert_eq!(plan.pending.len(), 1);
        let before = builder.charged_visits();
        let references = Arc::strong_count(&shared);
        let mut remaining = if host_exhausted { 0 } else { 1 };
        match plan.admit(
            &audits,
            &mut builder,
            &mut remaining,
            output_transfer_bytes(value.direct_children.len(), value.reachable.len()).unwrap(),
            identity_bound(SourceManifest::Raw(&value)).unwrap(),
        ) {
            Ok((_, preparation)) => {
                assert!(!host_exhausted);
                drop(preparation); // abandoned paid token must poison without copies
            }
            Err(error) => {
                assert!(host_exhausted);
                assert_eq!(error, "Args source inspection visit limit");
            }
        }
        assert_eq!(
            builder.charged_visits() - before,
            3,
            "two private preparation operations then pending native work precede host check"
        );
        assert_eq!(remaining, 0);
        assert!(audits.committed.is_empty());
        assert_eq!(audits.credit, 0);
        assert_eq!(Arc::strong_count(&shared), references);
        assert!(builder.charge_bytes(0).is_err());
    }
}

#[test]
fn transfer_backing_is_checked_for_empty_arrays_and_cannot_cover_retained_underpayment() {
    assert_eq!(output_transfer_bytes(0, 0).unwrap(), 0);
    assert!(output_transfer_bytes(usize::MAX, 0).is_err());
    assert!(output_transfer_bytes(0, usize::MAX).is_err());
    assert!(
        output_transfer_bytes(
            usize::MAX / size_of::<CallableModuleIdentity>(),
            usize::MAX / size_of::<CallableModuleSource>()
        )
        .is_err()
    );
    let mut manifest = super::super::super::tests::manifest("@@//pkg:source.bzl", [3; 32]);
    manifest.direct_children = Arc::from([]);
    manifest.reachable = Arc::from([]);
    let mut control = PublishedValueArenaBuilder::new();
    let mut control_audits = MappingAudits::default();
    let plan = forecast(&manifest, &control_audits, &mut control, &mut 65536).unwrap();
    let output = plan.output_bytes;
    drop(plan);
    let authority = convert(&manifest, &mut control_audits, &mut control, &mut 65536).unwrap();
    assert_eq!(
        output,
        super::super::tests::native_cost(&authority),
        "zero arrays leave all P in retained Q"
    );
    for beyond_peak in [false, true] {
        let audits = MappingAudits::default();
        let mut builder = PublishedValueArenaBuilder::new();
        let plan = forecast(&manifest, &audits, &mut builder, &mut 65536).unwrap();
        let transfer = if beyond_peak {
            plan.output_bytes + 1
        } else {
            1
        };
        match plan.admit(
            &audits,
            &mut builder,
            &mut 65536,
            transfer,
            identity_bound(SourceManifest::Raw(&manifest)).unwrap(),
        ) {
            Err(error) => {
                assert!(beyond_peak);
                assert_eq!(error, "Args source transfer byte underflow");
            }
            Ok((_, preparation)) => {
                assert!(!beyond_peak);
                let references = Arc::strong_count(&manifest.root.repository_mapping);
                let error = preparation.finish(authority.data().clone()).unwrap_err();
                assert!(
                    error.to_string().contains("source authority underpayment"),
                    "{error}"
                );
                assert_eq!(
                    Arc::strong_count(&manifest.root.repository_mapping),
                    references
                );
            }
        }
        assert!(audits.committed.is_empty());
        assert_eq!(audits.credit, 0);
        assert!(builder.charge_bytes(0).is_err());
    }
}
