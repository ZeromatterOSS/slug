use slug_build_api_v2::published_value::ImmutablePublicationInput;

use super::*;

fn fields(
    s: slug_build_api_v2::published_value::PublicationShape,
) -> (usize, usize, usize, usize, usize) {
    (s.nodes, s.edges, s.bytes, s.visits, s.depth)
}
#[tokio::test]
async fn borrowed_proof_precursor_matches_actual_metadata_and_refuses_before_guard_copies() {
    let (ordinary, mixed, guard) =
        crate::bzl_environment::environment_publication_tests::mixed_catalog_pair_with_guard()
            .await;
    let proof = mixed.inputs().source_proof().unwrap();
    let counts: Vec<_> = proof
        .plan
        .physical_payloads()
        .iter()
        .map(|p| match p {
            super::super::references::Payload::Catalog(c) => Arc::strong_count(c),
            _ => panic!("proof requires exact catalog-only physical closure"),
        })
        .collect();
    let mut work = Budget::default();
    let prepared = EnvironmentSourceFacts::prepare_publication(
        &proof.plan,
        proof.rows.len(),
        proof.rows.iter().map(|row| row.budget),
        &mut work,
    )
    .unwrap();
    assert_eq!(fields(prepared.full()), fields(proof.retained_shape()));
    let metadata = prepared
        .native
        .finish(
            proof
                .plan
                .physical_payloads()
                .iter()
                .map(|p| p.shared())
                .collect(),
        )
        .unwrap();
    assert_eq!(
        fields(metadata.intrinsic()),
        fields(proof.intrinsic_shape())
    );
    assert_eq!(metadata.children().len(), proof.nested_payloads().len());
    drop(metadata);
    for bytes in [false, true] {
        let mut work = Budget::default();
        if bytes {
            work.bytes = 64 * 1024 * 1024;
        } else {
            work.records = 65536;
        }
        let mut expected = work;
        let expected_error = expected
            .charge_with_context(
                super::budget_context(PeakPhase::SourceProofMetadata, FirstLedger::Work),
                1,
                std::mem::size_of::<Budget>(),
            )
            .unwrap_err();
        let refused = EnvironmentSourceFacts::prepare_publication(
            &proof.plan,
            proof.rows.len(),
            proof.rows.iter().map(|row| row.budget),
            &mut work,
        );
        assert!(refused.is_err());
        assert_eq!(refused.err().unwrap(), expected_error);
        assert_eq!(
            (work.records, work.bytes, work.depth),
            (expected.records, expected.bytes, expected.depth)
        );
        assert_eq!(
            counts,
            proof
                .plan
                .physical_payloads()
                .iter()
                .map(|p| match p {
                    super::super::references::Payload::Catalog(c) => Arc::strong_count(c),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>()
        );
    }
    assert!(Arc::ptr_eq(
        proof,
        ordinary.inputs().source_proof().unwrap()
    ));
    // End the exact count interval before allowing DICE state teardown.
    drop(guard);
}

#[tokio::test]
async fn multiple_actual_catalogs_keep_canonical_proof_budgets_and_events_equal() {
    let (ordinary, mixed) =
        crate::bzl_environment::environment_publication_tests::mixed_catalog_pair().await;
    let catalog_a = ordinary.inputs.catalog.as_ref().unwrap();
    let catalog_b = mixed.inputs.catalog.as_ref().unwrap();
    assert_eq!(catalog_a, catalog_b);
    assert!(!Arc::ptr_eq(catalog_a, catalog_b));
    let a = PreparedBzlEnvironment::seed(BzlEnvironmentMode::Builtin, Some(catalog_a)).unwrap();
    let b = PreparedBzlEnvironment::seed(BzlEnvironmentMode::Builtin, Some(catalog_b)).unwrap();
    let child = &ordinary.exports[0].module;
    let identity = crate::BzlModuleIdentity {
        label: CanonicalLabel::parse("@@_builtins//:catalog_bridge.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::builtin("fixture", "catalog_bridge.bzl")
            .unwrap(),
        repository_mapping: child.manifest.root.repository_mapping.clone(),
    };
    let source: Arc<[u8]> = Arc::from(b"load('child.bzl', 'apple_common_bazel')\nprint('bridge event')\nVALUE = apple_common_bazel\n".as_slice());
    let prepare = |env: &PreparedBzlEnvironment| {
        crate::bzl_module::semantic_evaluation::prepare(
            &identity,
            "catalog_bridge.bzl",
            &source,
            crate::bzl_module::BzlModuleContext::Bzlmod,
            env,
            std::iter::once(("child.bzl", child)),
        )
        .unwrap()
    };
    let ak = prepare(&a);
    let bk = prepare(&b);
    assert!(ak == bk);
    let left = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let right = dice::Dice::builder().build(dice::DetectCycles::Enabled);
    let mut lt = left.updater().commit().await;
    let mut rt = right.updater().commit().await;
    let av = lt.compute(&ak).await.unwrap();
    let bv = rt.compute(&bk).await.unwrap();
    let ai = av.result().unwrap().semantic_input_facts().unwrap();
    let bi = bv.result().unwrap().semantic_input_facts().unwrap();
    let ap = ai.source_proof.as_ref().unwrap();
    let bp = bi.source_proof.as_ref().unwrap();
    assert_eq!(ap.shared_inputs().len(), 1);
    assert_eq!(bp.shared_inputs().len(), 2);
    assert!(!Arc::ptr_eq(ap, bp));
    let counts = |v: Budget| (v.records, v.bytes, v.depth);
    assert_eq!(counts(ai.logical_budget()), counts(bi.logical_budget()));
    assert_eq!(counts(ap.budget()), counts(bp.budget()));
    assert!(ap == bp);
    assert_eq!(av.events(), bv.events());
    assert_eq!(av.events().events().len(), 1);
    assert!(bp.physical_budget().bytes > ap.physical_budget().bytes);
    assert!(bp.intrinsic_shape().bytes > ap.intrinsic_shape().bytes);
    assert_eq!(bp.shared_inputs().len(), bp.nested_payloads().len());
}
