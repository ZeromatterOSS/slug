use starlark::environment::FrozenModule;
use starlark::environment::Globals;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::eval::FunctionExportMode;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[derive(Debug, Allocative)]
struct TestPart {
    visits: usize,
}
impl StructuralFunctionOwnerPart for TestPart {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn eq_part(&self, other: &dyn StructuralFunctionOwnerPart) -> bool {
        other.as_any().is::<Self>()
    }
    fn hash_part(&self, state: &mut dyn Hasher) {
        state.write_u8(1);
    }
    fn retained_bytes(&self) -> usize {
        size_of::<Self>()
    }
    fn retained_shape(&self) -> PublicationShape {
        PublicationShape {
            nodes: 0,
            edges: 0,
            bytes: self.retained_bytes(),
            visits: self.visits,
            depth: 0,
        }
    }
}

fn exported_closures() -> FrozenModule {
    exported_closures_with_visits(1)
}

fn exported_closures_with_visits(visits: usize) -> FrozenModule {
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    eval.set_function_symbol_session(slug_loading_v2::function_owner::function_symbol_session(
        FunctionEvaluationOwner::new(
            FunctionOwnerDomain::BzlLoad,
            Arc::new(TestPart { visits: 0 }),
            Arc::new(TestPart { visits }),
        )
        .unwrap(),
    ))
    .unwrap();
    eval.set_function_export_mode(FunctionExportMode::TopLevelAssignments)
        .unwrap();
    eval.eval_module(
        AstModule::parse(
            "arbitrary.bzl",
            r#"
def factory(value):
    def inner():
        return value
    return inner
selected = factory('old')
saved = [selected]
selected = factory('new')
prior = saved[0]
"#
            .to_owned(),
            &Dialect::Bazel,
        )
        .unwrap(),
        &Globals::standard(),
    )
    .unwrap();
    drop(eval);
    module.freeze().unwrap()
}

#[test]
fn selected_implementation_witness_distinguishes_same_export_name_closures() {
    let first = exported_closures();
    let old = first.get("prior").unwrap();
    let new = first.get("selected").unwrap();
    let old = witness(old.value().unpack_frozen().unwrap()).unwrap();
    let new = witness(new.value().unpack_frozen().unwrap()).unwrap();
    assert_eq!(
        old.symbol, new.symbol,
        "language equality ignores creation ordinal after export"
    );
    assert_ne!(old.creation_ordinal, new.creation_ordinal);
    assert_ne!(
        old, new,
        "selected implementation identity must retain the exact closure"
    );
    let again = exported_closures();
    assert_eq!(
        old,
        witness(again.get("prior").unwrap().value().unpack_frozen().unwrap()).unwrap()
    );
    assert_eq!(
        new,
        witness(
            again
                .get("selected")
                .unwrap()
                .value()
                .unpack_frozen()
                .unwrap()
        )
        .unwrap()
    );
}

#[test]
fn selected_source_owner_is_declared_separately_from_local_input_facts() {
    let module = exported_closures_with_visits(30_000);
    let old = module.get("prior").unwrap();
    let new = module.get("selected").unwrap();
    let mut audit = Audit::default();
    audit
        .callable(old.value().unpack_frozen().unwrap())
        .unwrap();
    audit
        .callable(new.value().unpack_frozen().unwrap())
        .unwrap();
    assert_eq!(audit.native.owners().len(), 1);
    assert_eq!(
        &audit.native.owners()[0],
        witness(old.value().unpack_frozen().unwrap())
            .unwrap()
            .symbol
            .owner()
    );
    assert!(audit.native.local_shape().visits >= 30_000);
    assert!(
        audit.native.intrinsic_shape().visits < 100,
        "a configured input's local facts exclude its declared source owner"
    );
}

#[test]
fn configured_owner_borrowed_audit_refuses_transient_functions_and_excess_depth() {
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "unowned.bzl",
                "def f(): return 1\n".to_owned(),
                &Dialect::Bazel,
            )
            .unwrap(),
            &Globals::standard(),
        )
        .unwrap();
    let frozen = module.freeze().unwrap();
    let value = frozen.get("f").unwrap();
    assert!(
        Audit::default()
            .callable(value.value().unpack_frozen().unwrap())
            .unwrap_err()
            .contains("structural evaluation owner")
    );
    // Empty owner completion pays22 work. Two genuine completed source owners
    // still retain all65536 facts; witness/name/lookup work stays independent.
    let near_limit = exported_closures_with_visits(65_536 - 22);
    let selected = near_limit.get("selected").unwrap();
    let mut audit = Audit::default();
    audit
        .callable(selected.value().unpack_frozen().unwrap())
        .unwrap();
    assert_eq!(audit.native.charged_visits(), 4);
    assert_eq!(audit.native.intrinsic_shape().visits, 4);
    assert_eq!(audit.native.local_shape().visits, 65_536 - 22 + 4);
    assert_eq!(
        audit.native.owners()[0].retained_shape().visits,
        65_536 - 22
    );
    assert_eq!(audit.native.owners().len(), 1);
    let tail = exported_closures_with_visits(22);
    audit
        .callable(
            tail.get("selected")
                .unwrap()
                .value()
                .unpack_frozen()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(audit.native.charged_visits(), 8);
    assert_eq!(audit.native.intrinsic_shape().visits, 8);
    assert_eq!(audit.native.local_shape().visits, 65_536 + 8);
    assert_eq!(audit.native.owners().len(), 2);
    assert_eq!(
        audit
            .native
            .owners()
            .iter()
            .map(|o| o.intrinsic_shape().visits)
            .sum::<usize>(),
        65_536,
        "complete retained source-owner bodies still fill the same quota"
    );
    let mut nested = CoercedAttributeValue::None;
    for _ in 0..257 {
        nested = CoercedAttributeValue::Selector {
            branches: Arc::from([]),
            default: Some(Arc::new(nested)),
        };
    }
    assert!(
        Audit::default()
            .coerced(&nested, 0)
            .unwrap_err()
            .contains("depth")
    );
    assert!(
        Audit::default()
            .bytes(slug_build_api_v2::published_value::BYTE_LIMIT + 1)
            .unwrap_err()
            .contains("bytes")
    );
}

#[test]
fn joint_native_snapshot_preserves_aliases_across_input_rows() {
    let occurrence = || {
        AnalysisValue::provider(ProviderOccurrence::new(
            slug_build_api_v2::ProviderIdentity::builtin("UnrelatedInfo"),
            [(
                "values",
                AnalysisValue::list(vec![AnalysisValue::integer(1)]),
            )],
        ))
    };
    let one = occurrence();
    let a = AnalysisValue::tuple(vec![one.clone(), one]);
    let two = occurrence();
    let again = AnalysisValue::tuple(vec![two.clone(), two]);
    let distinct = AnalysisValue::tuple(vec![occurrence(), occurrence()]);
    assert!(a.publication_eq(&again));
    assert!(!a.publication_eq(&distinct));
    assert!(distinct.publication_eq(&AnalysisValue::tuple(vec![occurrence(), occurrence()])));
}

#[test]
fn runfiles_owner_audit_charges_linear_graph_work_and_owns_shared_node_guards() {
    fn package(name: &str) -> Arc<RunfilesPackageMetadata> {
        Arc::new(RunfilesPackageMetadata::new(
            slug_identity_v2::CanonicalLabel::parse(&format!("@@//{name}:owner"))
                .unwrap()
                .package()
                .clone(),
            Arc::new(slug_build_api_v2::RunfilesRepositoryMapping::empty()),
        ))
    }
    fn graph(
        width: usize,
    ) -> (
        Audit,
        RunfilesPackageDepset,
        std::sync::Weak<RunfilesPackageMetadata>,
    ) {
        let metadata = package("shared");
        let weak = Arc::downgrade(&metadata);
        // A singleton transitive child is inlined into its parent by the real
        // depset constructor. Two leaves retain a genuinely shared dense store.
        let shared = RunfilesPackageDepset::from_direct(
            DepsetOrder::Default,
            vec![metadata, package("shared_other")],
        )
        .unwrap();
        let branches = (0..width)
            .map(|index| {
                let branch = RunfilesPackageDepset::new(
                    DepsetOrder::Default,
                    vec![package(&format!("node_{index:05}"))],
                    vec![shared.clone()],
                )
                .unwrap();
                assert!(branch.successors().any(|successor| matches!(
                    successor,
                    slug_build_api_v2::DepsetSuccessor::Transitive(child)
                        if child.shares_node_with(&shared)
                )));
                branch
            })
            .collect();
        let root = RunfilesPackageDepset::new(DepsetOrder::Default, Vec::new(), branches).unwrap();
        let mut audit = Audit::default();
        audit.runfiles(&root, 0).unwrap();
        assert_eq!(audit.runfiles_stores.len(), width + 2);
        (audit, shared, weak)
    }

    let (small, _, _) = graph(512);
    let (medium, _, _) = graph(1024);
    let (mut large, retained, weak) = graph(2048);
    let a = small.native.local_shape();
    let b = medium.native.local_shape();
    let c = large.native.local_shape();
    assert_eq!(c.visits - b.visits, 2 * (b.visits - a.visits));
    assert_eq!(c.edges - b.edges, 2 * (b.edges - a.edges));
    assert_eq!(c.bytes - b.bytes, 2 * (b.bytes - a.bytes));
    assert_eq!(c.depth, 4);
    // Every branch reaches the common child, but its payload is audited once.
    assert!(c.visits < 20 * 2048);
    assert_eq!(large.runfiles_stores.len(), 2050);

    // The shared root can be audited again independently of the wide parent;
    // its whole store is already retained and its payload must not be revisited.
    assert!(weak.upgrade().is_some());
    large.runfiles(&retained, 0).unwrap();
    let repeated = large.native.local_shape();
    assert_eq!(repeated.visits, c.visits + 2);
    assert_eq!(repeated.edges, c.edges + 1);
    assert_eq!(repeated.bytes, c.bytes + size_of::<usize>());
    assert_eq!(large.runfiles_stores.len(), 2050);

    let separate = RunfilesPackageDepset::from_direct(
        DepsetOrder::Default,
        vec![package("shared"), package("shared_other")],
    )
    .unwrap();
    assert_eq!(
        retained, separate,
        "equal contents may still be distinct nodes"
    );
    assert_ne!(retained.node_key(), separate.node_key());
    large.runfiles(&separate, 0).unwrap();
    assert_eq!(large.runfiles_stores.len(), 2051);
    assert!(large.native.local_shape().visits > repeated.visits + 1);
    drop(retained);
    assert!(
        weak.upgrade().is_some(),
        "only memo guards now retain this payload"
    );
    drop(large);
    assert!(
        weak.upgrade().is_none(),
        "the phase memo must release its guards"
    );
}

#[test]
fn runfiles_owner_storage_preflight_precedes_payload_and_mapping_bytes_share_budget() {
    let payload = Arc::new(RunfilesPackageMetadata::new(
        slug_identity_v2::CanonicalLabel::parse("@@//payload:owner")
            .unwrap()
            .package()
            .clone(),
        Arc::new(slug_build_api_v2::RunfilesRepositoryMapping::new(
            Arc::from([]),
            Some(CompactString::from("mapping-owner".repeat(1024))),
        )),
    ));
    let packages = RunfilesPackageDepset::from_direct(DepsetOrder::Default, vec![payload]).unwrap();

    // Capture only the first O(1) physical forecast; refuse before a Node or
    // Value event, so this observation itself never inspects package payloads.
    let mut physical = None;
    let refused = packages.visit_storage_bounded(|event| match event {
        slug_build_api_v2::DepsetStorageEvent::Charge(charge) => {
            physical = Some(charge);
            Err(())
        }
        _ => panic!("the first physical charge must precede payload inspection"),
    });
    assert_eq!(refused, Err(()));
    let physical = physical.unwrap();
    assert!(physical.visits > 1);

    let mut limited = Audit::default();
    for _ in 0..65_534 {
        limited.native.charge_visit().unwrap();
    }
    assert!(
        limited
            .runfiles(&packages, 0)
            .unwrap_err()
            .contains("visits")
    );
    assert!(
        limited.runfiles_stores.is_empty(),
        "a refused store must not grow the memo"
    );
    // Only the enclosing edge and physical forecast were charged. Reading the
    // package/mapping fields would add their row and string bytes to this sum.
    assert_eq!(
        limited.native.charged_bytes(),
        size_of::<usize>() + physical.bytes
    );

    let mut byte_limited = Audit::default();
    byte_limited
        .native
        .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - 4096)
        .unwrap();
    assert!(
        byte_limited
            .runfiles(&packages, 0)
            .unwrap_err()
            .contains("bytes")
    );
    assert_eq!(
        byte_limited.runfiles_stores.len(),
        1,
        "the small store passed before its large mapping was refused"
    );

    // Failure belongs to this consumer attempt. A fresh owner can audit the
    // same immutable package, including its full mapping payload, normally.
    let mut fresh = Audit::default();
    fresh.runfiles(&packages, 0).unwrap();
    assert_eq!(fresh.runfiles_stores.len(), 1);
    assert!(fresh.native.charged_bytes() >= "mapping-owner".len() * 1024);
}
