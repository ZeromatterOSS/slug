use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use super::*;

fn node<'v>(
    module: &'v Module,
    depth: usize,
    successors: Vec<StarlarkDepsetSuccessorGen<Value<'v>>>,
) -> Value<'v> {
    module.heap().alloc_complex(StarlarkDepset {
        order: DepsetOrder::Default,
        element_type: Some("string".into()),
        occurrence: AnalysisDepsetOccurrence::new(),
        retained: None,
        depth,
        successors,
    })
}

#[test]
fn original_depset_freeze_is_stack_safe_and_preserves_diamond_occurrences() {
    let module = Module::new();
    let leaf = module.heap().alloc("leaf");
    let child = node(&module, 1, vec![StarlarkDepsetSuccessorGen::Direct(leaf)]);
    let split = node(&module, 1, vec![StarlarkDepsetSuccessorGen::Direct(leaf)]);
    let mut root = child;
    for depth in 2..=3499 {
        root = node(
            &module,
            depth,
            vec![
                StarlarkDepsetSuccessorGen::Direct(leaf),
                StarlarkDepsetSuccessorGen::Transitive(root),
            ],
        );
    }
    let diamond = node(
        &module,
        3500,
        vec![
            StarlarkDepsetSuccessorGen::Transitive(root),
            StarlarkDepsetSuccessorGen::Transitive(child),
            StarlarkDepsetSuccessorGen::Transitive(child),
            StarlarkDepsetSuccessorGen::Transitive(split),
        ],
    );
    module.set("ROOT", root);
    module.set("DIAMOND", diamond);
    let frozen = module.freeze().unwrap();
    let root = frozen.get("ROOT").unwrap();
    assert_eq!(
        StarlarkDepset::depth_from_value(root.value().to_value()),
        Some(3499)
    );
    let diamond = frozen.get("DIAMOND").unwrap();
    let parts = StarlarkDepset::parts_from_value(diamond.value().to_value()).unwrap();
    let values: Vec<_> = parts
        .5
        .into_iter()
        .map(|s| match s {
            StarlarkDepsetSuccessorGen::Direct(v) | StarlarkDepsetSuccessorGen::Transitive(v) => v,
        })
        .collect();
    assert_eq!(values[0].identity(), root.value().to_value().identity());
    assert_eq!(values[1].identity(), values[2].identity());
    assert_ne!(values[1].identity(), values[3].identity());
}

#[test]
fn original_depset_freeze_preserves_direct_backreference_and_foreign_child() {
    let producer = Module::new();
    let child = node(
        &producer,
        1,
        vec![StarlarkDepsetSuccessorGen::Direct(
            producer.heap().alloc("foreign"),
        )],
    );
    producer.set("CHILD", child);
    let producer = producer.freeze().unwrap();
    let child = producer.get("CHILD").unwrap();
    let module = Module::new();
    module.frozen_heap().add_reference(producer.frozen_heap());
    Evaluator::new(&module)
        .eval_module(
            AstModule::parse(
                "unrelated-backreference.bzl",
                "def back():
    return ROOT
ROOT = depset([back])
"
                .into(),
                &Dialect::Standard,
            )
            .unwrap(),
            &GlobalsBuilder::standard()
                .with(|builder| builder.set("depset", AnalysisBuiltinCallable::new("depset")))
                .build(),
        )
        .unwrap();
    let foreign = node(
        &module,
        2,
        vec![StarlarkDepsetSuccessorGen::Transitive(
            child.value().to_value(),
        )],
    );
    module.set("FOREIGN", foreign);
    let frozen = module.freeze().unwrap();
    drop(child);
    drop(producer);
    let back = frozen.get("back").unwrap();
    let consumer = Module::new();
    consumer.frozen_heap().add_reference(frozen.frozen_heap());
    let returned = Evaluator::new(&consumer)
        .eval_function(back.value().to_value(), &[], &[])
        .unwrap();
    let root = frozen.get("ROOT").unwrap();
    assert_eq!(returned.identity(), root.value().to_value().identity());
    let leaves = StarlarkDepset::direct_from_value(returned).unwrap();
    assert_eq!(leaves[0].identity(), back.value().to_value().identity());
    let foreign = frozen.get("FOREIGN").unwrap();
    let parts = StarlarkDepset::parts_from_value(foreign.value().to_value()).unwrap();
    let StarlarkDepsetSuccessorGen::Transitive(child) = parts.5[0] else {
        panic!("transitive")
    };
    let direct = StarlarkDepset::direct_from_value(child).unwrap();
    assert_eq!(direct[0].unpack_str(), Some("foreign"));
}
