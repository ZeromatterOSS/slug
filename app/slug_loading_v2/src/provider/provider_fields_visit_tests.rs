//! Borrowed field preflight covers actual moving and frozen provider owners.
use super::*;
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
