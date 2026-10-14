//! Actual VM depset graph lifecycle; separate phases retain the default stack.
use super::*;

const BUILD: &str = r#"
def build():
    shared = depset(transitive = [depset([0]), depset([1])])
    value = shared
    for i in range(2, 3500):
        value = depset(direct = [i], transitive = [value])
    return value, shared
ROOT, SHARED = build()
ALIAS = ROOT
SHARED_ALIAS = SHARED
EMPTY = depset()
"#;
const COLLECT: &str = r#"
def hold(root, shared):
    alias = root
    collect()
    if not root or not shared or EMPTY or alias != root:
        fail("actual depset changed during collection")
    return alias
CAPTURED = hold(ROOT, SHARED)
RESULT = (len(ROOT.to_list()), bool(ROOT), bool(EMPTY))
"#;

fn eval_live(module: &Module, source: &str) -> Result<(), String> {
    eval_controlled(module, source, false)
}
fn eval_controlled(module: &Module, source: &str, preparation: bool) -> Result<(), String> {
    let mut evaluator = Evaluator::new(module);
    // Isolate collection from fixture construction; the fresh COLLECT evaluator
    // retains automatic collection and invokes the actual initialized GC helper.
    if preparation {
        evaluator.disable_gc();
    }
    evaluator
        .eval_module(
            AstModule::parse(
                "actual_depset_lifecycle.bzl",
                source.to_owned(),
                &Dialect::Standard,
            )
            .unwrap(),
            &globals(),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn assert_vm_chain<'v>(root: Value<'v>, shared: Value<'v>) {
    assert_eq!(StarlarkDepset::depth_from_value(root), Some(3500));
    let mut cursor = root;
    for expected in (3..=3500).rev() {
        let (_, _, _, retained, depth, successors) =
            StarlarkDepset::parts_from_value(cursor).unwrap();
        assert!(
            retained.is_none(),
            "fixture must retain VM successors, not a native facade"
        );
        assert_eq!(depth, expected);
        let mut child = None;
        for successor in successors {
            if let StarlarkDepsetSuccessorGen::Transitive(value) = successor {
                assert!(child.replace(value).is_none());
            }
        }
        cursor = child.expect("actual transitive VM edge");
    }
    assert!(
        cursor.ptr_eq(shared),
        "shared root must be the original deep-chain child"
    );
    assert_eq!(StarlarkDepset::depth_from_value(shared), Some(2));
}
fn refuses_parent(module: &Module) {
    let error = eval_live(
        module,
        "INVALID = depset(direct = [3500], transitive = [ROOT])\n",
    )
    .unwrap_err();
    assert!(
        error.contains("depset depth 3501 exceeds limit (3500)"),
        "{error}"
    );
}

#[test]
fn original_deep_depset_gc_keeps_vm_edges_aliases_and_depth_limit() {
    eprintln!("DEPSET_LIFECYCLE phase=build_gc_only");
    let module = Module::new();
    eval_controlled(&module, BUILD, true).unwrap();
    eprintln!("DEPSET_LIFECYCLE phase=forced_gc");
    eval_live(&module, COLLECT).unwrap();
    eprintln!("DEPSET_LIFECYCLE phase=after_gc");
    let root = module.get("ROOT").unwrap();
    let shared = module.get("SHARED").unwrap();
    assert!(root.ptr_eq(module.get("ALIAS").unwrap()));
    assert!(root.ptr_eq(module.get("CAPTURED").unwrap()));
    assert!(shared.ptr_eq(module.get("SHARED_ALIAS").unwrap()));
    assert_vm_chain(root, shared);
    assert_eq!(
        module.get("RESULT").unwrap().to_repr(),
        "(3500, True, False)"
    );
    refuses_parent(&module);
    // End borrows before module drop. This test deliberately never freezes.
    eprintln!("DEPSET_LIFECYCLE phase=drop_mutable");
    drop(module);
    eprintln!("DEPSET_LIFECYCLE phase=done_gc_only");
}

#[test]
fn original_deep_depset_freeze_drop_import_and_consumer_gc_keep_aliases() {
    eprintln!("DEPSET_LIFECYCLE phase=build_full_lifecycle");
    let producer = Module::new();
    eval_controlled(&producer, BUILD, true).unwrap();
    eprintln!("DEPSET_LIFECYCLE phase=producer_gc");
    eval_live(&producer, COLLECT).unwrap();
    eprintln!("DEPSET_LIFECYCLE phase=producer_freeze");
    let frozen = producer.freeze().unwrap();
    eprintln!("DEPSET_LIFECYCLE phase=after_producer_freeze");
    // Keep only non-owning frozen tokens for identity comparisons. After
    // import + producer drop, the consumer is the sole external heap owner.
    let original_root = frozen.get("ROOT").unwrap().value().unpack_frozen().unwrap();
    let original_shared = frozen
        .get("SHARED")
        .unwrap()
        .value()
        .unpack_frozen()
        .unwrap();
    assert_vm_chain(original_root.to_value(), original_shared.to_value());
    let consumer = Module::new();
    consumer.import_public_symbols(&frozen);
    drop(frozen);
    // Imports are private host bindings. Assign public consumer aliases before
    // host inspection, retaining the actual imported values and heap ownership.
    eval_live(
        &consumer,
        "ROOT = ROOT\nSHARED = SHARED\nALIAS = ALIAS\nSHARED_ALIAS = SHARED_ALIAS\n",
    )
    .unwrap();
    eprintln!("DEPSET_LIFECYCLE phase=consumer_gc_after_producer_drop");
    eval_live(&consumer, COLLECT).unwrap();
    assert_eq!(
        consumer.get("RESULT").unwrap().to_repr(),
        "(3500, True, False)"
    );
    assert!(
        consumer
            .get("ROOT")
            .unwrap()
            .ptr_eq(original_root.to_value())
    );
    assert!(
        consumer
            .get("CAPTURED")
            .unwrap()
            .ptr_eq(original_root.to_value())
    );
    assert!(
        consumer
            .get("SHARED")
            .unwrap()
            .ptr_eq(original_shared.to_value())
    );
    refuses_parent(&consumer);
    eprintln!("DEPSET_LIFECYCLE phase=consumer_freeze");
    let consumer = consumer.freeze().unwrap();
    assert!(
        consumer
            .get("ALIAS")
            .unwrap()
            .value()
            .ptr_eq(original_root.to_value())
    );
    assert!(
        consumer
            .get("CAPTURED")
            .unwrap()
            .value()
            .ptr_eq(original_root.to_value())
    );
    assert!(
        consumer
            .get("SHARED_ALIAS")
            .unwrap()
            .value()
            .ptr_eq(original_shared.to_value())
    );
    assert_vm_chain(
        consumer.get("ROOT").unwrap().value().to_value(),
        consumer.get("SHARED").unwrap().value().to_value(),
    );
    eprintln!("DEPSET_LIFECYCLE phase=drop_frozen");
    drop(consumer);
    eprintln!("DEPSET_LIFECYCLE phase=done_freeze_import");
}
