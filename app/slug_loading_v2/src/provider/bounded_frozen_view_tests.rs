use std::cell::Cell;

use slug_build_api_v2::AnalysisArtifactRoot;
use slug_build_api_v2::AnalysisValueKind;
use slug_build_api_v2::FeatureFlagInfo;
use slug_build_api_v2::OutputGroupInfo;
use slug_build_api_v2::RunEnvironmentInfo as NativeRunEnvironmentInfo;
use slug_build_api_v2::TemplateVariableInfo;
use slug_identity_v2::ApparentLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;

use super::*;

fn observed_arena_bytes(allocate: impl Fn(&FrozenHeap)) -> usize {
    let heap = FrozenHeap::new();
    // A recycled chunk can initially have only enough room for one object.
    // Compare object consumption after a reservation stays open.
    (0..8)
        .find_map(|_| {
            let (reserved, available) = (heap.allocated_bytes(), heap.available_bytes());
            allocate(&heap);
            (heap.allocated_bytes() == reserved).then(|| available - heap.available_bytes())
        })
        .expect("an ordinary allocation in an open chunk")
}

#[test]
fn bounded_user_provider_borrows_canonical_fields_and_spends_exact_storage() {
    let id = ProviderId::new("@@//pkg:defs.bzl", "ProbeInfo").unwrap();
    let spelling = "z_long_field_spelling_bigger_than_inline_storage";
    let mut long_name = CompactString::new(spelling);
    long_name.reserve(512);
    let occurrence = ProviderOccurrence::new(
        ProviderIdentity::user(id.dupe()),
        [
            (long_name, AnalysisValue::string("later")),
            (CompactString::new("a"), AnalysisValue::string("first")),
        ],
    )
    .with_schema_classification(true);
    let arena = observed_arena_bytes(|heap| {
        alloc_starlark_user_provider(heap, id.dupe(), true, std::iter::empty());
    });
    let bytes = arena
        + 4 * size_of::<CompactString>()
        + 2 * size_of::<FrozenValue>()
        + 2 * size_of::<AtomicUsize>()
        + spelling.len();
    for available in [bytes - 1, bytes] {
        let budget = ExecutionBudget::new(50_000, available, 8);
        let heap = FrozenHeap::new();
        let fields = [
            heap.alloc("exact first child"),
            heap.alloc("exact later child"),
        ];
        let called = Cell::new(0);
        let result = alloc_budgeted_starlark_user_provider::<ExecutionLimit>(
            &heap,
            &budget,
            &occurrence,
            |value| {
                let index = called.get();
                called.set(index + 1);
                let AnalysisValueKind::String(text) = value.kind() else {
                    panic!("fixture field must be a string");
                };
                assert_eq!(text, ["first", "later"][index]);
                Ok(fields[index])
            },
        );
        assert_eq!(called.get(), 2);
        if available == bytes {
            let result = result.unwrap().to_value();
            assert_eq!(loading_provider_id(result), Some(id.dupe()));
            assert_eq!(starlark_user_provider_has_schema(result), Some(true));
            let mut count = 0;
            assert!(
                visit_starlark_provider_fields(result, &mut |name, value| {
                    assert_eq!(name, ["a", spelling][count]);
                    assert!(value.ptr_eq(fields[count].to_value()));
                    count += 1;
                    Ok(())
                })
                .unwrap()
            );
            assert_eq!(count, 2);
            assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        } else {
            assert_eq!(result.unwrap_err(), ExecutionLimit::AllocationBytes);
            assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
        }
    }

    let heap = FrozenHeap::new();
    let no_scratch = ExecutionBudget::new(50_000, 0, 8);
    assert_eq!(
        alloc_budgeted_starlark_user_provider::<ExecutionLimit>(
            &heap,
            &no_scratch,
            &occurrence,
            |_| panic!("projected before admitting field storage"),
        )
        .unwrap_err(),
        ExecutionLimit::AllocationBytes
    );
    let builtin = ProviderOccurrence::empty(ProviderIdentity::builtin("ToolchainInfo"));
    let budget = ExecutionBudget::new(50_000, 1 << 20, 8);
    let refused = ExecutionLimit::UnsupportedOperation {
        operation: "provider materialization",
        value_type: builtin.value_type().as_str(),
    };
    assert_eq!(
        alloc_budgeted_starlark_user_provider::<ExecutionLimit>(
            &heap,
            &budget,
            &builtin,
            |_| panic!("projected a built-in through the user view"),
        )
        .unwrap_err(),
        refused
    );
    assert_eq!(budget.check(), Err(refused));
}

#[test]
fn bounded_depset_preserves_the_owned_successor_vector_and_occurrence() {
    let order = DepsetOrder::Postorder;
    let retained = AnalysisDepset::new(
        order,
        vec![AnalysisValue::string("direct leaf")],
        Vec::new(),
    )
    .unwrap();
    let bytes = observed_arena_bytes(|heap| {
        alloc_starlark_depset(heap, retained.dupe(), Vec::new());
    });
    for available in [bytes - 1, bytes] {
        let heap = FrozenHeap::new();
        let leaf = heap.alloc("direct leaf");
        let successors = vec![StarlarkDepsetSuccessorGen::Direct(leaf)];
        let storage = successors.as_ptr();
        let budget = ExecutionBudget::new(50_000, available, 8);
        let result = alloc_budgeted_starlark_depset(&heap, &budget, retained.dupe(), successors);
        if available == bytes {
            let result = result
                .unwrap()
                .downcast_frozen_ref::<FrozenStarlarkDepset>()
                .unwrap();
            assert_eq!(result.order, order);
            assert_eq!(result.element_type.as_deref(), Some("string"));
            assert_eq!(result.depth, retained.depth());
            assert_eq!(result.occurrence, retained.occurrence());
            assert_eq!(result.retained.as_ref(), Some(&retained));
            assert_eq!(result.successors.as_ptr(), storage);
            let StarlarkDepsetSuccessorGen::Direct(actual) = result.successors[0] else {
                panic!("direct successor changed kind");
            };
            assert!(actual.to_value().ptr_eq(leaf.to_value()));
            assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        } else {
            assert_eq!(result.unwrap_err(), ExecutionLimit::AllocationBytes);
        }
    }
}

#[test]
fn bounded_frozen_label_preserves_canonical_mapping_provenance() {
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("active-map").unwrap());
    let label = ApparentLabel::parse("@visible//pkg/sub:artifact")
        .unwrap()
        .resolve(&mapping);
    let bytes = observed_arena_bytes(|heap| {
        alloc_frozen_starlark_label(heap, label.clone());
    }) + ["visible", "pkg/sub", "artifact", "active-map"]
        .map(str::len)
        .into_iter()
        .sum::<usize>();
    for available in [bytes - 1, bytes] {
        let heap = FrozenHeap::new();
        let budget = ExecutionBudget::new(50_000, available, 8);
        let result = alloc_budgeted_frozen_starlark_label(&heap, &budget, &label);
        if available == bytes {
            let result = result.unwrap();
            let actual = starlark_label_ref(result.to_value()).unwrap();
            assert_eq!(actual, &label);
            assert_eq!(actual.mapping_id(), Some(mapping.id()));
            assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        } else {
            assert_eq!(result.unwrap_err(), ExecutionLimit::AllocationBytes);
            assert_eq!(heap.allocated_bytes(), 0);
        }
    }
}

fn check_wrapper_admission(
    extra: usize,
    ordinary: impl Fn(&FrozenHeap) -> FrozenValue,
    bounded: impl Fn(&FrozenHeap, &ExecutionBudget) -> Result<FrozenValue, ExecutionLimit>,
    check: impl Fn(FrozenValue),
) {
    let bytes = extra
        + observed_arena_bytes(|heap| {
            ordinary(heap);
        });
    let heap = FrozenHeap::new();
    let short = ExecutionBudget::new(50_000, bytes - 1, 8);
    assert_eq!(
        bounded(&heap, &short).unwrap_err(),
        ExecutionLimit::AllocationBytes
    );
    assert_eq!(short.check(), Err(ExecutionLimit::AllocationBytes));
    assert_eq!(heap.allocated_bytes(), 0);
    let exact = ExecutionBudget::new(50_000, bytes, 8);
    check(bounded(&heap, &exact).unwrap());
    assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
}

#[test]
fn bounded_fixed_native_wrappers_retain_their_existing_payloads() {
    let flag = FeatureFlagInfo::new("enabled");
    check_wrapper_admission(
        0,
        |heap| alloc_frozen_feature_flag_info(heap, flag.dupe()),
        |heap, budget| alloc_budgeted_frozen_feature_flag_info(heap, budget, &flag),
        |value| assert_eq!(feature_flag_info(value.to_value()).as_ref(), Some(&flag)),
    );

    let entries = SmallMap::from_iter([(
        CompactString::const_new("KEY"),
        CompactString::const_new("long retained content"),
    )]);
    let template = TemplateVariableInfo::new(entries.clone());
    check_wrapper_admission(
        0,
        |heap| alloc_frozen_template_variable_info(heap, template.dupe()),
        |heap, budget| alloc_budgeted_frozen_template_variable_info(heap, budget, &template),
        |value| {
            let restored = template_variable_info_ref(value.to_value()).unwrap();
            assert!(std::ptr::eq(restored.variables(), template.variables()));
        },
    );

    let environment =
        NativeRunEnvironmentInfo::new(entries, Arc::from(["INHERIT".into(), "INHERIT".into()]));
    check_wrapper_admission(
        0,
        |heap| alloc_frozen_run_environment_info(heap, environment.dupe()),
        |heap, budget| alloc_budgeted_frozen_run_environment_info(heap, budget, &environment),
        |value| {
            let restored = run_environment_info_ref(value.to_value()).unwrap();
            assert!(std::ptr::eq(
                restored.environment(),
                environment.environment()
            ));
            assert!(std::ptr::eq(
                restored.inherited_environment(),
                environment.inherited_environment()
            ));
        },
    );

    let repository = "external_root";
    let root = AnalysisArtifactRoot::Source(CanonicalRepoName::new(repository).unwrap());
    check_wrapper_admission(
        repository.len(),
        |heap| crate::alloc_frozen_artifact_root(heap, root.clone()),
        |heap, budget| crate::alloc_budgeted_frozen_artifact_root(heap, budget, &root),
        |value| {
            assert_eq!(
                crate::artifact_root_value_ref(value.to_value()),
                Some(&root)
            )
        },
    );
}

#[test]
fn bounded_toolchain_fields_keep_canonical_order_and_original_values() {
    let occurrence = ProviderOccurrence::new(
        ProviderIdentity::builtin("ToolchainInfo"),
        [
            ("z", AnalysisValue::string("last")),
            ("a", AnalysisValue::string("first")),
        ],
    );
    let extra = 4 * size_of::<CompactString>()
        + 2 * size_of::<FrozenValue>()
        + 2 * size_of::<AtomicUsize>();
    check_wrapper_admission(
        extra,
        |heap| StarlarkToolchainInfo::alloc(heap, std::iter::empty()),
        |heap, budget| {
            let first = FrozenValue::new_bool(true);
            let last = FrozenValue::new_none();
            StarlarkToolchainInfo::alloc_budgeted(heap, budget, &occurrence, |value| {
                match value.kind() {
                    AnalysisValueKind::String("first") => Ok(first),
                    AnalysisValueKind::String("last") => Ok(last),
                    _ => panic!("unexpected toolchain field"),
                }
            })
        },
        |value| {
            let values = StarlarkToolchainInfo::fields_from_value(value.to_value()).unwrap();
            assert_eq!(values[0].0, "a");
            assert_eq!(values[0].1.unpack_bool(), Some(true));
            assert_eq!(values[1].0, "z");
            assert!(values[1].1.is_none());
        },
    );
}

#[test]
fn bounded_output_groups_keep_utf16_order_and_shared_fresh_empty_value() {
    let shape = OutputGroupInfo::new([
        ("\u{e000}", AnalysisDepset::empty(DepsetOrder::Postorder)),
        ("\u{10000}", AnalysisDepset::empty(DepsetOrder::Preorder)),
    ])
    .unwrap();
    let empty = AnalysisDepset::empty(DepsetOrder::Default);
    let wrapper = observed_arena_bytes(|heap| {
        StarlarkOutputGroupInfo::alloc(heap, std::iter::empty());
    });
    let child = observed_arena_bytes(|heap| {
        alloc_starlark_depset(heap, empty.dupe(), Vec::new());
    });
    let bytes = wrapper
        + child
        + 4 * size_of::<CompactString>()
        + 2 * size_of::<FrozenValue>()
        + 2 * size_of::<AtomicUsize>();
    for available in [bytes - 1, bytes] {
        let heap = FrozenHeap::new();
        let supplied = alloc_starlark_depset(&heap, empty.dupe(), Vec::new());
        let budget = ExecutionBudget::new(50_000, available, 8);
        let result = StarlarkOutputGroupInfo::alloc_budgeted::<ExecutionLimit>(
            &heap,
            &budget,
            &shape,
            |retained| {
                assert_eq!(retained, &empty);
                Ok(supplied)
            },
        );
        if available == bytes {
            let values =
                StarlarkOutputGroupInfo::fields_from_value(result.unwrap().to_value()).unwrap();
            assert_eq!(values[0].0, "\u{10000}");
            assert_eq!(values[1].0, "\u{e000}");
            assert!(values[0].1.ptr_eq(values[1].1));
            assert!(!values[0].1.ptr_eq(supplied.to_value()));
            assert_eq!(budget.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
        } else {
            assert_eq!(result.unwrap_err(), ExecutionLimit::AllocationBytes);
            assert_eq!(budget.check(), Err(ExecutionLimit::AllocationBytes));
        }
    }
    let budget = ExecutionBudget::new(50_000, bytes, 8);
    let heap = FrozenHeap::new();
    let wrong = FrozenValue::new_none();
    let expected = ExecutionLimit::UnsupportedOperation {
        operation: "OutputGroupInfo materialization",
        value_type: wrong.to_value().get_type(),
    };
    assert_eq!(
        StarlarkOutputGroupInfo::alloc_budgeted::<ExecutionLimit>(&heap, &budget, &shape, |_| {
            Ok(wrong)
        })
        .unwrap_err(),
        expected
    );
    assert_eq!(budget.check(), Err(expected));
    assert_eq!(heap.allocated_bytes(), 0);
}

#[test]
fn borrowed_compact_copy_admits_fresh_spelling_bytes() {
    let text = "snow雪".repeat(16);
    let bytes = text.len();
    let short = ExecutionBudget::new(100, bytes - 1, 8);
    assert_eq!(
        copy_budgeted_compact_str(&short, &text),
        Err(ExecutionLimit::AllocationBytes)
    );
    let exact = ExecutionBudget::new(100, bytes, 8);
    assert_eq!(
        copy_budgeted_compact_str(&exact, &text).unwrap().as_str(),
        text
    );
    assert_eq!(exact.charge(0, 1), Err(ExecutionLimit::AllocationBytes));
    assert_eq!(
        copy_budgeted_compact_str(&ExecutionBudget::new(0, 0, 8), ""),
        Err(ExecutionLimit::Work)
    );
}
