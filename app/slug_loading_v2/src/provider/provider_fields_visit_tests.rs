//! Borrowed field visits and lookup cover actual moving and frozen provider owners.
use starlark::environment::Module;

use super::*;

fn user_provider<'v>(
    module: &'v Module,
    schema: Option<&[&str]>,
    named: &[(&str, Value<'v>)],
) -> Value<'v> {
    let schema = match schema {
        Some(fields) => UserProviderSchema::List(
            fields
                .iter()
                .map(|name| CompactString::new(*name))
                .collect(),
        ),
        None => UserProviderSchema::Schemaless,
    };
    let callable = module.frozen_heap().alloc(FrozenUserProviderCallable {
        id: ProviderId::new("@@//:independent.bzl", "Probe").unwrap(),
        schema,
    });
    Evaluator::new(module)
        .eval_function(callable.to_value(), &[], named)
        .unwrap()
}

#[test]
fn bounded_user_provider_lookup_preserves_live_field_identity_and_absence() {
    let budget = Arc::new(ExecutionBudget::new(100_000, 1 << 20, 16).with_bounded_operations());
    let module = Module::new_with_execution_budget(budget.clone());
    let heap = module.heap();
    let payload = heap.alloc("a retained field payload");
    for schema in [Some(&["none", "present", "unset"][..]), None] {
        let provider = user_provider(
            &module,
            schema,
            &[("present", payload), ("none", Value::new_none())],
        );
        assert!(
            provider
                .get_attr("present", heap)
                .unwrap()
                .unwrap()
                .ptr_eq(payload)
        );
        assert!(provider.get_attr("none", heap).unwrap().unwrap().is_none());
        for missing in ["unset", "unknown"] {
            assert!(provider.get_attr(missing, heap).unwrap().is_none());
        }
    }
    assert_eq!(budget.check(), Ok(()));
}

#[test]
fn bounded_user_provider_lookup_charges_retained_rows_and_equal_length_names() {
    let long_name = "z".repeat(513);
    let source = Module::new();
    let payload = source.frozen_heap().alloc("a retained field payload");
    let named = &[
        ("none", Value::new_none()),
        (long_name.as_str(), payload.to_value()),
    ];
    source.set(
        "DECLARED",
        user_provider(&source, Some(&["none", "unset", &long_name]), named),
    );
    source.set("OPEN", user_provider(&source, None, named));
    source.set(
        "MATERIALIZED",
        alloc_starlark_user_provider(
            source.frozen_heap(),
            ProviderId::new("@@//:independent.bzl", "Materialized").unwrap(),
            true,
            [
                (CompactString::new("none"), FrozenValue::new_none()),
                (CompactString::new(&long_name), payload),
            ],
        )
        .to_value(),
    );
    let source = source.freeze().unwrap();

    for owner in ["DECLARED", "OPEN", "MATERIALIZED"] {
        let provider = source.get(owner).unwrap();
        for (name, work, expected) in [
            // One lookup, one unit for the query bytes, and two retained rows.
            ("unset", 4, None),
            // One lookup, three units for query bytes, two rows, and three
            // units to compare the equal-length name.
            (long_name.as_str(), 9, Some(payload)),
        ] {
            for available in [work, work - 1] {
                let budget =
                    Arc::new(ExecutionBudget::new(available, 0, 16).with_bounded_operations());
                let module = Module::new_with_execution_budget(budget.clone());
                let provider = provider.owned_value(module.frozen_heap());
                let result = provider.get_attr(name, module.heap());
                if available == work {
                    let got = result.unwrap();
                    match expected {
                        Some(value) => assert!(got.unwrap().ptr_eq(value.to_value())),
                        None => assert!(got.is_none()),
                    }
                    assert_eq!(budget.charge(1, 0), Err(ExecutionLimit::Work));
                } else {
                    let error = result.unwrap_err();
                    assert!(matches!(
                        error.kind(),
                        starlark::ErrorKind::Other(cause)
                            if cause.downcast_ref::<ExecutionLimit>() == Some(&ExecutionLimit::Work)
                    ));
                    assert_eq!(budget.check(), Err(ExecutionLimit::Work));
                }
            }
        }
    }
}

#[test]
fn borrowed_provider_fields_visit_mutable_frozen_schema_and_stop_on_error() {
    let module = starlark::environment::Module::new();
    let id = ProviderId::new("@@//:independent.bzl", "Declared").unwrap();
    let user = module.heap().alloc_complex(LoadingStarlarkUserProviderGen {
        id,
        fields: LoadingProviderFieldsGen::Schemaful {
            schema: Arc::from([CompactString::new("unset"), CompactString::new("present")]),
            values: SmallMap::from_iter([(1u32, module.heap().alloc("payload"))]),
        },
    });
    let tool = StarlarkToolchainInfo::alloc_value(
        module.heap(),
        [
            (CompactString::new("first"), module.heap().alloc("one")),
            (CompactString::new("second"), module.heap().alloc("two")),
        ],
    );
    let mut names = Vec::new();
    assert!(
        visit_starlark_provider_fields(user, &mut |name, _| {
            names.push(name.to_owned());
            Ok(())
        })
        .unwrap()
    );
    assert_eq!(names, ["present"]);
    let mut calls = 0;
    assert_eq!(
        visit_starlark_provider_fields(tool, &mut |_, _| {
            calls += 1;
            Err("field budget exhausted".into())
        })
        .unwrap_err(),
        "field budget exhausted"
    );
    assert_eq!(calls, 1);
    assert!(
        !visit_starlark_provider_fields(module.heap().alloc("unrecognized"), &mut |_, _| panic!(
            "unknown objects must not invoke visitor"
        ))
        .unwrap()
    );
    module.set("user", user);
    module.set("tool", tool);
    let frozen = module.freeze().unwrap();
    for (name, expected) in [("user", 1), ("tool", 2)] {
        let owned = frozen.get(name).unwrap();
        let mut count = 0;
        assert!(
            visit_starlark_provider_fields(owned.value().to_value(), &mut |_, _| {
                count += 1;
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(count, expected);
    }
}

#[test]
fn borrowed_depset_frontier_charges_header_and_stops_before_owned_projection() {
    let module = starlark::environment::Module::new();
    let child = module.heap().alloc_complex(StarlarkDepset {
        order: DepsetOrder::Default,
        element_type: Some("string".into()),
        occurrence: AnalysisDepsetOccurrence::new(),
        retained: None,
        depth: 1,
        successors: vec![StarlarkDepsetSuccessorGen::Direct(
            module.heap().alloc("payload"),
        )],
    });
    let parent = module.heap().alloc_complex(StarlarkDepset {
        order: DepsetOrder::Default,
        element_type: Some("string".into()),
        occurrence: AnalysisDepsetOccurrence::new(),
        retained: None,
        depth: 2,
        successors: vec![StarlarkDepsetSuccessorGen::Transitive(child)],
    });
    let mut header = None;
    assert!(
        StarlarkDepset::visit_metadata_from_value(parent, &mut |name, count, retained| {
            header = Some((name.unwrap().to_owned(), count, retained));
            Ok(())
        })
        .unwrap()
    );
    assert_eq!(header, Some(("string".into(), 1, false)));
    let mut visits = 0;
    assert_eq!(
        StarlarkDepset::visit_successors_from_value(parent, &mut |_| {
            visits += 1;
            Err("successor budget".into())
        })
        .unwrap_err(),
        "successor budget"
    );
    assert_eq!(visits, 1);
    assert!(
        !StarlarkDepset::visit_successors_from_value(
            module.heap().alloc("unknown"),
            &mut |_| panic!("unknown value visited")
        )
        .unwrap()
    );
    module.set("ROOT", parent);
    let frozen = module.freeze().unwrap();
    let owned = frozen.get("ROOT").unwrap();
    let mut leaves = 0;
    StarlarkDepset::visit_successors_from_value(owned.value().to_value(), &mut |child| {
        StarlarkDepset::visit_successors_from_value(child, &mut |_| {
            leaves += 1;
            Ok(())
        })?;
        Ok(())
    })
    .unwrap();
    assert_eq!(leaves, 1);
}
