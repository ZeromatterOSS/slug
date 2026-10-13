//! Native ingress truth follows Bazel 9.2 8220c619 Depset.java283–290.
use slug_build_api_v2::DepsetOrder;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::Module;

use super::*;

const ORDERS: [DepsetOrder; 4] = [
    DepsetOrder::Default,
    DepsetOrder::Postorder,
    DepsetOrder::Preorder,
    DepsetOrder::Topological,
];

fn source() -> AnalysisArtifact {
    AnalysisArtifact::Source(CanonicalLabel::parse("@@//pkg:source.txt").unwrap())
}

#[test]
fn opaque_retained_depset_truth_agrees_with_native_emptiness() {
    for order in ORDERS {
        let mut depsets = vec![AnalysisDepset::empty(order)];
        for leaf in [
            AnalysisValue::string(""),
            AnalysisValue::boolean(false),
            AnalysisValue::artifact(source()),
        ] {
            depsets.push(AnalysisDepset::new(order, vec![leaf], vec![]).unwrap());
        }
        for native in depsets {
            let retained = {
                let heap = FrozenHeap::new();
                // A retained native value can intentionally hide its successors.
                // Truth must still distinguish empty from nonempty metadata.
                let value = alloc_starlark_depset(&heap, native.dupe(), vec![]);
                let (_, element_type, occurrence, _, depth, successors) =
                    StarlarkDepset::parts_from_value(value.to_value()).unwrap();
                assert!(successors.is_empty());
                assert_eq!(element_type.is_none(), native.is_empty());
                assert_eq!(value.to_value().to_bool(), !native.is_empty());
                assert_eq!(depth, native.depth());
                assert_eq!(occurrence, native.occurrence());
                let lowered = AnalysisValueLowerer::default()
                    .lower(value.to_value(), "$opaque")
                    .unwrap();
                assert!(AnalysisValue::depset(native.dupe()).publication_eq(&lowered));
                lowered
            };
            // The native owner survives disposal of its producer heap and is
            // rematerialized with its actual successors by the generic bridge.
            let heap = FrozenHeap::new();
            let restored = AnalysisValueMaterializer::new(&heap)
                .value(&retained)
                .unwrap();
            assert_eq!(restored.to_value().to_bool(), !native.is_empty());
            let lowered = AnalysisValueLowerer::default()
                .lower(restored.to_value(), "$restored")
                .unwrap();
            assert!(retained.publication_eq(&lowered));
        }
    }
}

#[test]
fn generic_transitive_depset_truth_survives_roundtrip_freeze_gc_and_heap_drop() {
    for order in ORDERS {
        let retained = {
            let heap = FrozenHeap::new();
            let empty =
                AnalysisDepset::new(order, vec![], vec![AnalysisDepset::empty(order)]).unwrap();
            let shared =
                AnalysisDepset::new(order, vec![AnalysisValue::string("")], vec![]).unwrap();
            let branch = |name| {
                AnalysisDepset::new(
                    order,
                    vec![AnalysisValue::string(name)],
                    vec![shared.dupe()],
                )
                .unwrap()
            };
            let root =
                AnalysisDepset::new(order, vec![], vec![branch("left"), branch("right")]).unwrap();
            assert!(empty.is_empty());
            assert!(!shared.is_empty());
            assert!(!root.is_empty());
            let expected = AnalysisValue::tuple(vec![
                AnalysisValue::depset(empty),
                AnalysisValue::depset(shared),
                AnalysisValue::depset(root.dupe()),
                AnalysisValue::depset(root),
            ]);
            let values = AnalysisValueMaterializer::new(&heap)
                .value(&expected)
                .unwrap();
            let elements = TupleRef::from_value(values.to_value()).unwrap();
            assert!(!elements.content()[0].to_bool());
            assert!(elements.content()[1].to_bool());
            assert!(elements.content()[2].to_bool());
            let lowered = AnalysisValueLowerer::default()
                .lower(values.to_value(), "$producer")
                .unwrap();
            assert!(expected.publication_eq(&lowered));
            lowered
        };
        let module = Module::new();
        let restored = AnalysisValueMaterializer::new(module.frozen_heap())
            .value(&retained)
            .unwrap();
        module.set("values", restored.to_value());
        let lowered = AnalysisValueLowerer::default()
            .lower(restored.to_value(), "$restored")
            .unwrap();
        assert!(retained.publication_eq(&lowered));
        let frozen = module.freeze().unwrap();
        let mut consumer = starlark::assert::Assert::new();
        consumer.disable_static_typechecking();
        consumer.module_add("retained", frozen);
        consumer.pass(
            r#"
load("retained", "values")
empty, false_leaf, transitive, alias = values
garbage_collect()
assert_eq(bool(empty), False)
assert_eq(not empty, True)
assert_eq(bool(false_leaf), True)
assert_eq(bool(transitive), True)
assert_eq(transitive, alias)
assert_eq("present" if transitive else "missing", "present")
"#,
        );
    }
}

#[test]
fn string_and_symlink_materializers_preserve_truth_through_native_roundtrip() {
    for order in ORDERS {
        let (strings, symlinks) = {
            let heap = FrozenHeap::new();
            let mut materializer = AnalysisValueMaterializer::new(&heap);
            let direct = Depset::new(order, vec![String::new()], vec![]).unwrap();
            let strings = [
                Depset::new(order, vec![], vec![]).unwrap(),
                direct.clone(),
                Depset::new(order, vec![], vec![direct]).unwrap(),
            ]
            .iter()
            .map(|native| {
                let value = materializer.file_depset(native);
                assert_eq!(value.to_value().to_bool(), !native.is_empty());
                AnalysisValueLowerer::default()
                    .lower(value.to_value(), "$strings")
                    .unwrap()
            })
            .collect::<Vec<_>>();
            let direct = RunfilesSymlinkDepset::new(
                order,
                vec![RunfilesSymlink::new("link", source())],
                vec![],
            )
            .unwrap();
            let symlinks = [
                RunfilesSymlinkDepset::new(order, vec![], vec![]).unwrap(),
                direct.clone(),
                RunfilesSymlinkDepset::new(order, vec![], vec![direct]).unwrap(),
            ]
            .iter()
            .map(|native| {
                let value = materializer.symlink_depset(native);
                assert_eq!(value.to_value().to_bool(), !native.is_empty());
                let lowered = lower_runfiles_symlink_depset(value.to_value(), "$symlinks").unwrap();
                assert_eq!(lowered, *native);
                lowered
            })
            .collect::<Vec<_>>();
            (strings, symlinks)
        };
        let module = Module::new();
        let mut materializer = AnalysisValueMaterializer::new(module.frozen_heap());
        let string_values = strings
            .iter()
            .map(|native| {
                let value = materializer.value(native).unwrap();
                let lowered = AnalysisValueLowerer::default()
                    .lower(value.to_value(), "$restored_strings")
                    .unwrap();
                assert!(native.publication_eq(&lowered));
                value
            })
            .collect::<Vec<_>>();
        let symlink_values = symlinks
            .iter()
            .map(|native| {
                let value = materializer.symlink_depset(native);
                assert_eq!(
                    lower_runfiles_symlink_depset(value.to_value(), "$restored_symlinks").unwrap(),
                    *native
                );
                value
            })
            .collect::<Vec<_>>();
        module.set(
            "strings",
            module
                .frozen_heap()
                .alloc(AllocTuple(string_values))
                .to_value(),
        );
        module.set(
            "symlinks",
            module
                .frozen_heap()
                .alloc(AllocTuple(symlink_values))
                .to_value(),
        );
        drop(materializer);
        let frozen = module.freeze().unwrap();
        let mut consumer = starlark::assert::Assert::new();
        consumer.disable_static_typechecking();
        consumer.module_add("retained", frozen);
        consumer.pass(
            r#"
load("retained", "strings", "symlinks")
garbage_collect()
assert_eq([bool(value) for value in strings], [False, True, True])
assert_eq([bool(value) for value in symlinks], [False, True, True])
"#,
        );
    }
}
