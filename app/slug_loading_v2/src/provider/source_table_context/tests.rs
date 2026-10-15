use super::*;
use crate::bzl_module::semantic_evaluation::budget::RECORD_LIMIT;
#[test]
fn context_capture_is_cumulative_prepaid_and_refused_factory_has_no_fallback() {
    let context = BzlEvaluationContext::new("//:defs.bzl");
    let before = context.publication_work.borrow().records;
    let first = context.source_table().unwrap();
    let second = context.source_table().unwrap();
    assert_eq!(context.publication_work.borrow().records, before + 10);
    assert!(Arc::ptr_eq(first.raw(), second.raw()));
    context.publication_work.borrow_mut().records = RECORD_LIMIT;
    let count = Arc::strong_count(first.raw());
    assert!(context.source_table().is_err());
    assert_eq!(Arc::strong_count(first.raw()), count);
    assert_eq!(context.publication_work.borrow().records, RECORD_LIMIT + 1);
    // An originally refused preparation keeps readonly provenance, but cannot capture.
    let raw = context.source_table.raw().clone();
    let mut work = Budget {
        records: RECORD_LIMIT,
        bytes: 0,
        depth: 0,
    };
    let refused = SourceTableCapture::from_raw(raw.clone(), &mut work);
    assert!(Arc::ptr_eq(refused.raw(), &raw));
    assert!(refused.certified(&mut Budget::default()).is_err());
    // No RefMut survives either API return, so a later invocation can borrow safely.
    assert!(context.publication_work.try_borrow_mut().is_ok());
}

#[test]
fn effective_context_seeds_exact_completed_producer_work_and_reuses_its_pair() {
    let id = BzlEvaluationContext::new("//:defs.bzl")
        .source_identity()
        .clone();
    let mut id = id;
    id.source = slug_build_api_v2::ModuleSourceOrigin::Workspace("//:defs.bzl".into());
    let manifest = crate::BzlLoadManifest {
        root: id.clone(),
        direct_children: Arc::from([]),
        reachable: Arc::from([BzlModuleSourceProvenance::new(id, [9; 32])]),
        fingerprint: [8; 32],
    };
    let env = crate::bzl_environment::PreparedBzlEnvironment::uninjected(
        crate::bzl_environment::BzlEnvironmentMode::BzlmodBootstrap,
    )
    .unwrap();
    let source = crate::BzlModuleSourceContext::new(&manifest, &env, std::iter::empty()).unwrap();
    let seed = source.producer_work();
    let context = BzlEvaluationContext::from_source_context(&source).unwrap();
    assert_eq!(context.publication_work.borrow().records, seed.records + 8);
    assert_eq!(context.publication_work.borrow().depth, seed.depth);
    let expected_bytes = std::mem::size_of::<BzlEvaluationContext>()
        + std::mem::size_of::<SourceTableCapture>()
        + std::mem::size_of::<RefCell<Budget>>()
        + std::mem::size_of::<CertifiedSourceTable>();
    assert_eq!(
        context.publication_work.borrow().bytes,
        seed.bytes + expected_bytes
    );
    let table = context.source_table().unwrap();
    assert!(Arc::ptr_eq(table.raw(), source.sources()));
    let factory = SourceTableCapture::from_raw(source.sources().clone(), &mut Budget::default());
    let fresh = factory.certified(&mut Budget::default()).unwrap();
    // The factory itself retains its certified pair; end that producer too.
    drop(factory);
    let full = table.declarations()[0].retained_shape();
    let actual = fresh.declarations()[0].retained_shape();
    assert_eq!(
        (full.nodes, full.edges, full.visits, full.depth),
        (actual.nodes, actual.edges, actual.visits, actual.depth)
    );
    assert_eq!(
        full.bytes - actual.bytes,
        source.sources()[0].0.len().max(32) + std::mem::size_of::<usize>()
            - source.sources()[0].0.capacity()
    );
    let weak = Arc::downgrade(table.raw());
    drop(fresh);
    drop(source);
    drop(context);
    assert!(weak.upgrade().is_some());
    drop(table);
    assert!(weak.upgrade().is_none());
}
