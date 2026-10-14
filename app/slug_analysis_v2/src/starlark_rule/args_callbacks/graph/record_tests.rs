//! Actual VM encounters compose one optional row without changing graph facts.
use slug_build_api_v2::AnalysisValue;
use slug_build_api_v2::published_value::*;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

#[derive(Debug)]
struct Originals(usize);
impl PublishedOriginalBinding for Originals {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn root_count(&self) -> usize {
        self.0
    }
    fn imported_arenas(&self) -> &[PublishedValueArena] {
        &[]
    }
}

#[test]
fn value_record_literal_and_memo_keep_host_and_native_work_distinct() {
    let module = Module::new();
    let mut lowerer = AnalysisValueLowerer::default();
    let mut b = PublicationBuilder::new(&module);
    let id = b.add_value(Value::new_none(), &mut lowerer).unwrap();
    assert_eq!(
        b.graph.charged_visits(),
        2,
        "encounter plus unchanged native audit"
    );
    assert_eq!(b.remaining, CALLABLE_GRAPH_NODE_LIMIT - 2);
    let bytes = b.graph.charged_bytes();
    let roots = b.roots.len();
    assert_eq!(b.add_value(Value::new_none(), &mut lowerer).unwrap(), id);
    assert_eq!(b.graph.charged_visits(), 3, "memo encounter is still paid");
    assert_eq!(b.remaining, CALLABLE_GRAPH_NODE_LIMIT - 3);
    assert_eq!(b.graph.charged_bytes(), bytes);
    assert_eq!(b.roots.len(), roots);
    assert_eq!(b.values.len(), 1);
    assert_eq!(b.graph.local_shape().nodes, 1);
    let (native, roots) = b.finish();
    let producer_bytes = native.charged_bytes();
    let actual = native.seal(Arc::new(Originals(roots.len()))).unwrap();
    let mut reference = PublishedValueArenaBuilder::new();
    let row = reference.reserve().unwrap(); // Public reserve remains paid.
    let atom = reference.native_atom(AnalysisValue::none()).unwrap();
    reference
        .set_node(row, PublishedNode::Native(atom))
        .unwrap();
    reference
        .set_original_slot(row, PublishedRootSlot::new(0).unwrap())
        .unwrap();
    let reference_bytes = reference.charged_bytes();
    let reference = reference.seal(Arc::new(Originals(1))).unwrap();
    let mut map_forecast = PublishedValueArenaBuilder::new();
    storage::PeakCredit::default()
        .map::<ValueIdentity<'_>, PublishedNodeId>(&mut map_forecast, 1)
        .unwrap();
    assert_eq!(
        actual.shape().bytes - reference.shape().bytes,
        producer_bytes - reference_bytes - map_forecast.charged_bytes(),
        "only dead lookup map is excluded; original root and native forecasts stay retained"
    );
    assert_eq!(
        actual.shape().visits,
        reference.shape().visits,
        "canonical facts unchanged"
    );
    assert!(
        actual
            .value(id)
            .unwrap()
            .publication_eq(&reference.value(row).unwrap())
    );
    for (remaining, allowed) in [(2, true), (1, false)] {
        let mut b = PublicationBuilder::new(&module);
        while b.graph.charged_visits() < NODE_LIMIT - remaining {
            b.graph.charge_visit().unwrap();
        }
        let result = b.add_value(Value::new_none(), &mut lowerer);
        assert_eq!(result.is_ok(), allowed);
        assert_eq!(b.graph.charged_visits(), NODE_LIMIT);
        if allowed {
            assert_eq!(b.roots.len(), 1);
        } else {
            assert!(b.graph.charge_bytes(0).is_err());
        }
    }
}

#[test]
fn value_record_real_list_aliases_and_in_progress_cycle_keep_actual_ids() {
    let module = Module::new();
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "record.star",
                "shared = []\npayload = [shared, shared, []]\ncycle = []\ncycle.append(cycle)\n"
                    .into(),
                &Dialect::Standard,
            )
            .unwrap(),
            &GlobalsBuilder::new().build(),
        )
        .unwrap();
    let payload = module.get("payload").unwrap();
    let mut lowerer = AnalysisValueLowerer::default();
    let mut b = PublicationBuilder::new(&module);
    let root = b.add_value(payload, &mut lowerer).unwrap();
    assert_eq!(
        b.roots.len(),
        3,
        "root, actual shared list, independent equal list"
    );
    assert_eq!(b.graph.local_shape().nodes, 3);
    assert_eq!(
        b.graph.charged_visits(),
        7,
        "four encounters plus three container walks"
    );
    assert_eq!(b.remaining, CALLABLE_GRAPH_NODE_LIMIT - 7);
    let (native, roots) = b.finish();
    let arena = native.seal(Arc::new(Originals(roots.len()))).unwrap();
    let PublishedNode::List(range) = arena.node(root).unwrap() else {
        panic!("list")
    };
    let edges = arena.edges(*range).unwrap();
    assert_eq!(edges.len(), 3);
    assert_eq!(edges[0], edges[1]);
    assert_ne!(edges[0], edges[2]);
    let cycle = module.get("cycle").unwrap();
    let mut b = PublicationBuilder::new(&module);
    let id = b.add_value(cycle, &mut lowerer).unwrap();
    assert_eq!(b.values.get(&cycle.identity()), Some(&id));
    assert_eq!(b.roots.len(), 1);
    assert_eq!(b.graph.local_shape().nodes, 1);
    assert_eq!(
        b.graph.local_shape().edges,
        2,
        "one stored edge and its complete row-range membership"
    );
    assert_eq!(
        b.graph.charged_visits(),
        3,
        "encounter, container, paid in-progress memo hit"
    );
    let (native, roots) = b.finish();
    assert!(matches!(
        native.seal(Arc::new(Originals(roots.len()))),
        Err(PublishedValueError::Unsupported("cyclic native projection"))
    ));
    // No cyclic VM freeze is needed to prove its exact native structural refusal.
}

#[test]
fn value_record_host_and_scratch_failure_precede_copy_and_memo_growth() {
    let module = Module::new();
    let mut lowerer = AnalysisValueLowerer::default();
    let mut b = PublicationBuilder::new(&module);
    b.add_value(Value::new_none(), &mut lowerer).unwrap();
    b.remaining = 0;
    let before = b.graph.charged_visits();
    let nodes = b.graph.local_shape().nodes;
    let error = b.add_value(Value::new_none(), &mut lowerer).unwrap_err();
    assert_eq!(error, "Args callable inspection visit limit");
    assert_eq!(b.graph.charged_visits(), before + 1);
    assert_eq!(b.graph.local_shape().nodes, nodes);
    assert_eq!(b.roots.len(), 1);
    assert_eq!(b.values.len(), 1);
    assert!(
        b.graph.charge_bytes(0).is_err(),
        "explicit host abort poisons without another visit"
    );
    let mut b = PublicationBuilder::new(&module);
    assert!(
        b.value(Value::new_none(), &mut lowerer, DEPTH_LIMIT + 1)
            .is_err()
    );
    assert_eq!(b.graph.charged_visits(), 0);
    assert_eq!(b.remaining, CALLABLE_GRAPH_NODE_LIMIT);
    assert!(b.roots.is_empty());
    assert!(b.values.is_empty());
    let mut forecast = PublishedValueArenaBuilder::new();
    storage::PeakCredit::default()
        .map::<ValueIdentity<'_>, PublishedNodeId>(&mut forecast, 1)
        .unwrap();
    let map_bytes = forecast.charged_bytes();
    for allowance in [1, map_bytes] {
        let mut b = PublicationBuilder::new(&module);
        b.graph.charge_bytes(BYTE_LIMIT - allowance).unwrap();
        assert!(b.add_value(Value::new_none(), &mut lowerer).is_err());
        assert_eq!(b.graph.charged_visits(), 1);
        assert_eq!(b.remaining, CALLABLE_GRAPH_NODE_LIMIT - 1);
        assert_eq!(
            b.graph.local_shape().nodes,
            0,
            "row and slot storage follow BOTH credits"
        );
        assert!(b.roots.is_empty());
        assert_eq!(b.roots.capacity(), 0);
        assert!(b.values.is_empty());
        assert!(b.graph.charge_bytes(0).is_err());
        if allowance == map_bytes {
            assert_eq!(
                b.graph.charged_bytes(),
                BYTE_LIMIT,
                "map fits; root credit refuses before row allocation"
            );
        }
    }
}
