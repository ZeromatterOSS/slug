use super::*;
use crate::published_value::builder::import_accounting::tests::finish;

fn names(count: usize) -> Arc<[compact_str::CompactString]> {
    (0..count)
        .map(|i| format!("field{i}").into())
        .collect::<Vec<_>>()
        .into()
}
fn row(kind: usize, count: usize) -> PublishedNode {
    let range = PublishedEdgeRange {
        start: 0,
        len: count as u32,
    };
    let names = names(count);
    match kind {
        0 => PublishedNode::Function(PublishedFunction {
            symbol: PublishedFunctionSymbol::new(
                evaluation_owner(),
                FunctionSymbolName::Exported("f".into()),
            ),
            code: PublishedCode {
                source: source(0),
                name: "f".into(),
                definition: crate::CallableDefinitionSpan { start: 0, end: 1 },
                dependencies: PublishedEdgeRange { start: 0, len: 0 },
                globals: Arc::from([]),
            },
            parameters: Arc::from([]),
            return_annotation: None,
            capture_names: names,
            captures: range,
        }),
        1 => PublishedNode::ProviderConstructor(PublishedProviderConstructor {
            id: crate::ProviderId::new("//ordinary:api.bzl", "Info").unwrap(),
            schema: PublishedProviderSchema::List(names),
            init: None,
        }),
        2 => PublishedNode::Struct {
            names,
            values: range,
        },
        3 => PublishedNode::Provider(PublishedProvider {
            identity: crate::ProviderIdentity::builtin("ToolchainInfo"),
            has_schema: false,
            names,
            values: range,
            files_to_run: None,
        }),
        _ => unreachable!(),
    }
}

#[test]
fn immutable_name_lengths_keep_all_four_full_fact_formulas_and_paid_work() {
    for count in [0usize, 1, 97] {
        for kind in 0..4 {
            let row = row(kind, count);
            let source_facts = if let PublishedNode::Function(f) = &row {
                f.code.source.fact_count()
            } else {
                0
            };
            let edge = PublishedEdge::Local(PublishedNodeId::new(0).unwrap());
            let mut work = 0;
            let (facts, _) = local_visits(
                &[Some(row)],
                &vec![edge; count],
                &[None],
                &[],
                0,
                0,
                &mut work,
                &mut 0,
                &mut 0,
                &mut false,
            )
            .unwrap();
            // Original formulas: row header + every metadata name + ENTIRE
            // backing edge array + slot, plus actual complete source/reference.
            let fixed = [6 + source_facts, 3, 2, 3][kind];
            assert_eq!(facts.total, fixed + 2 * count, "kind{kind}/count{count}");
            assert_eq!(facts.source, source_facts);
            assert_eq!(facts.native, 0);
            assert_eq!(facts.other(), facts.total - source_facts);
            assert_eq!(work, [10, 5, 4, 5][kind] + count);
        }
    }
}

#[test]
fn length_read_work_bytes_full_fact_boundaries_and_poison_are_independent() {
    for count in [0usize, 1, 32] {
        let values = vec![(); count];
        for (remaining, succeeds) in [(1, true), (0, false)] {
            let mut work = NODE_LIMIT - remaining;
            let mut bytes = 0;
            let mut failed = false;
            let mut meter = Meter {
                work: &mut work,
                bytes: &mut bytes,
                scratch: &mut 0,
                failed: &mut failed,
                retained: 0,
                source: 0,
                native: 0,
            };
            assert_eq!(meter.count_records(&values).is_ok(), succeeds);
            assert_eq!(*meter.work, NODE_LIMIT);
            assert_eq!(meter.retained, if succeeds { count } else { 0 });
            assert_eq!(
                *meter.bytes,
                if succeeds {
                    std::mem::size_of::<usize>()
                } else {
                    0
                }
            );
            assert_eq!(*meter.failed, !succeeds);
            if !succeeds {
                assert!(meter.count_records(&[] as &[()]).is_err());
            }
        }
        for (allowance, succeeds) in [
            (std::mem::size_of::<usize>(), true),
            (std::mem::size_of::<usize>() - 1, false),
        ] {
            let mut bytes = BYTE_LIMIT - allowance;
            let mut meter = Meter {
                work: &mut 0,
                bytes: &mut bytes,
                scratch: &mut 0,
                failed: &mut false,
                retained: 0,
                source: 0,
                native: 0,
            };
            assert_eq!(meter.count_records(&values).is_ok(), succeeds);
            assert_eq!(*meter.work, 1);
            assert_eq!(meter.retained, if succeeds { count } else { 0 });
            assert_eq!(*meter.failed, !succeeds);
        }
    }
    // Primitive ledger test: actual immutable slices reach the complete fact
    // boundary without a large allocation; this is not a forged arena proof.
    let mut meter = Meter {
        work: &mut 0,
        bytes: &mut 0,
        scratch: &mut 0,
        failed: &mut false,
        retained: 0,
        source: 0,
        native: 0,
    };
    for _ in 0..RETAINED_FACT_LIMIT / 256 {
        meter.count_records(&[(); 256]).unwrap();
    }
    assert_eq!(meter.retained, RETAINED_FACT_LIMIT);
    assert!(matches!(
        meter.count_records(&[()]),
        Err(PublishedValueError::Limit("retained facts"))
    ));
    assert_eq!(meter.retained, RETAINED_FACT_LIMIT);
    assert!(*meter.failed);
    assert!(meter.count_records(&[] as &[()]).is_err());
    // Existing checked-add primitive rejects arithmetic overflow too; no body
    // certificate or publication is manufactured from these scalar unit inputs.
    let mut exhausted = NODE_LIMIT;
    let mut meter = Meter {
        work: &mut exhausted,
        bytes: &mut 0,
        scratch: &mut 0,
        failed: &mut false,
        retained: 0,
        source: 0,
        native: 0,
    };
    assert_eq!(
        meter.work().unwrap_err(),
        PublishedValueError::Limit("retained certificate work")
    );
    assert!(*meter.failed);
    let mut meter = Meter {
        work: &mut 0,
        bytes: &mut 0,
        scratch: &mut 0,
        failed: &mut false,
        retained: 0,
        source: 0,
        native: 0,
    };
    meter.add(1).unwrap();
    assert!(meter.add(usize::MAX).is_err());
    assert_eq!(meter.retained, 1);
    assert!(*meter.failed);
}

#[path = "name_count_tests/real_graph.rs"]
mod real_graph;
