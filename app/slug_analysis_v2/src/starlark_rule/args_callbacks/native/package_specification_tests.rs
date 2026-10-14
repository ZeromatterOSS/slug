//! Complete native receivers keep occurrence aliases and execute across GC.
use slug_build_api_v2::PackageGroupContents;
use slug_build_api_v2::PackageGroupContentsInput;
use slug_build_api_v2::PackageSpecificationInfo;
use slug_build_api_v2::published_value::PublishedValueArenaBuilder;
use slug_identity_v2::CanonicalLabel;
use starlark::values::ValueLike;

use super::*;

fn value(module: &Module, package: Option<&str>) -> starlark::values::FrozenValue {
    let exact_positive = package
        .into_iter()
        .map(|raw| CanonicalLabel::parse(raw).unwrap().package().clone())
        .collect();
    let group = Arc::new(PackageGroupContents::new(PackageGroupContentsInput {
        exact_positive,
        ..Default::default()
    }));
    crate::package_specification::alloc(
        module.frozen_heap(),
        PackageSpecificationInfo::new(group, []).unwrap(),
    )
}

#[test]
fn native_package_bound_wrappers_preserve_occurrences_receiver_and_original_gc_calls() {
    let producer = Module::new();
    let info = value(&producer, Some("@@//allowed:target"));
    producer.set("info", info.to_value());
    evaluate(
        &producer,
        r#"
first = info.contains
alias = first
second = info.contains
def mapper(value):
    if not collect(): fail('GC must reclaim test garbage')
    return str(info.contains(value)) + ":" + str(first(value)) + ":" + str(alias(value)) + ":" + str(second(value))
"#,
        &GlobalsBuilder::standard().with(gc_helpers).build(),
    );
    let first = producer.get("first").unwrap();
    let second = producer.get("second").unwrap();
    assert!(!first.ptr_eq(second));
    let inspection = starlark::eval::inspect_native_bound_method(first).unwrap();
    assert_eq!(
        super::super::native::NativeApis::certify_bound_method(inspection)
            .unwrap()
            .canonical(),
        "starlark.PackageSpecificationInfo.contains"
    );
    let mut lowerer = AnalysisValueLowerer::default();
    let mut builder = PublicationBuilder::new(&producer);
    let receiver = builder.add_value(info.to_value(), &mut lowerer).unwrap();
    let a = builder.add_callable(first, &mut lowerer).unwrap();
    let alias = builder
        .add_callable(producer.get("alias").unwrap(), &mut lowerer)
        .unwrap();
    let b = builder.add_callable(second, &mut lowerer).unwrap();
    assert_eq!(a, alias);
    assert_ne!(a, b);
    let (graph, roots) = builder.finish();
    for id in [a, b] {
        let PublishedNode::Intrinsic(row) = graph.node(id).unwrap() else {
            panic!("actual bound native wrapper");
        };
        assert_eq!(
            row.receiver,
            Some(slug_build_api_v2::published_value::PublishedEdge::Local(
                receiver
            ))
        );
    }
    producer.set_extra_value(producer.heap().alloc(AllocTuple(roots)));
    drop(lowerer);
    let frozen = producer.freeze().unwrap();
    let published = FrozenPublisher::new(graph, frozen.owned_extra_value().unwrap()).unwrap();
    let first_row = published.graph.value(a).unwrap();
    assert!(first_row.same_original(&published.graph.value(alias).unwrap()));
    assert!(!first_row.same_original(&published.graph.value(b).unwrap()));
    assert!(
        published
            .value(a)
            .unwrap_err()
            .contains("native API descriptors")
    );
    let runtime = StarlarkRuntimeMapper::new(frozen.get("mapper").unwrap());
    drop(first_row);
    drop(published);
    drop(frozen);
    for _ in 0..2 {
        assert_eq!(
            runtime
                .map_batch(
                    &[
                        AnalysisValue::string("//allowed:other"),
                        AnalysisValue::string("//other:target"),
                    ],
                    &NoopArgsDirectoryContext
                )
                .unwrap(),
            ["True:True:True:True", "False:False:False:False"]
        );
    }
}

#[test]
fn borrowed_package_preflight_refuses_complete_hidden_payload_before_lowering() {
    let module = Module::new();
    let empty = value(&module, None);
    let expanded = value(&module, Some(&format!("@@//{}:target", "long".repeat(128))));
    let audit_bytes = |value| {
        let mut graph = PublishedValueArenaBuilder::new();
        let mut remaining = 65536;
        assert!(
            super::super::native::pure_candidate(&module, &mut graph, &mut remaining, value, 3)
                .unwrap()
        );
        graph.charged_bytes()
    };
    let baseline = audit_bytes(empty.to_value());
    let extra = audit_bytes(expanded.to_value()) - baseline;
    assert!(extra > 512);
    for (candidate, succeeds) in [(empty, true), (expanded, false)] {
        let mut graph = PublishedValueArenaBuilder::new();
        graph
            .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - baseline - extra / 2)
            .unwrap();
        let mut remaining = 65536;
        let checked = super::super::native::pure_candidate(
            &module,
            &mut graph,
            &mut remaining,
            candidate.to_value(),
            3,
        );
        if succeeds {
            assert!(checked.unwrap());
        } else {
            assert!(checked.unwrap_err().contains("bytes"));
            assert!(
                graph.charge_visit().is_err(),
                "failed preflight poisons reuse"
            );
        }
    }
}
