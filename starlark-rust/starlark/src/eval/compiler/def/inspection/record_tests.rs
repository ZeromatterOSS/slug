//! One fixed record shares primary-span admission; dynamic work stays checked.
use super::*;
use crate::codemap::Span;
use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
use crate::eval::runtime::inlined_frame::InlinedFrame;
use crate::eval::runtime::inlined_frame::InlinedFrames;
use crate::values::FrozenHeap;

fn source(heap: &FrozenHeap, name: &str) -> crate::values::FrozenRef<'static, CodeMap> {
    heap.alloc_any(CodeMap::new(name.to_owned(), "same bytes\n".to_owned()))
}
fn span(file: crate::values::FrozenRef<'static, CodeMap>) -> FrameSpan {
    FrameSpan::new(FrozenFileSpan::new(file, Span::default()))
}
#[test]
fn fixed_record_primary_span_has_one_admission_and_guard_precedes_dispatch() {
    let heap = FrozenHeap::new();
    let file = source(&heap, "record.bzl");
    let node = 17;
    for (remaining, maximum_depth, depth, succeeds) in
        [(1, 0, 0, true), (0, 0, 0, false), (1, 0, 1, false)]
    {
        let mut budget = InspectionBudget {
            remaining_nodes: remaining,
            maximum_depth,
        };
        let mut calls = 0;
        {
            let mut visitor = |event| {
                let DefDependency::Source(found) = event else {
                    panic!("unexpected dependency")
                };
                assert_eq!(found.file.id(), file.id());
                calls += 1;
                Ok(())
            };
            let mut walker = Walker {
                module: OriginalModule::Missing,
                last_source: None,
                budget: &mut budget,
                visitor: &mut visitor,
            };
            let result = walker.record(&node, span(file), depth);
            assert_eq!(result.is_ok(), succeeds);
            if let Ok(found) = result {
                assert!(std::ptr::eq(found, &node));
            }
            assert_eq!(walker.last_source.is_some(), succeeds);
        }
        assert_eq!(calls, usize::from(succeeds));
        assert_eq!(budget.remaining_nodes, if succeeds { 0 } else { remaining });
    }
}
#[test]
fn source_consumer_error_prevents_record_dispatch_and_source_state_update() {
    let heap = FrozenHeap::new();
    let file = source(&heap, "record.bzl");
    let mut budget = InspectionBudget {
        remaining_nodes: 2,
        maximum_depth: 0,
    };
    let mut calls = 0;
    let mut visitor = |_| {
        calls += 1;
        if calls == 1 {
            Err(crate::Error::new_other(anyhow::anyhow!("source rejected")))
        } else {
            Ok(())
        }
    };
    let mut walker = Walker {
        module: OriginalModule::Missing,
        last_source: None,
        budget: &mut budget,
        visitor: &mut visitor,
    };
    assert!(
        walker
            .record(&7, span(file), 0)
            .unwrap_err()
            .to_string()
            .contains("source rejected")
    );
    assert!(walker.last_source.is_none());
    // Retrying the same source must invoke the visitor again, not suppress it.
    assert_eq!(*walker.record(&9, span(file), 0).unwrap(), 9);
    assert_eq!(walker.budget.remaining_nodes, 0);
    drop(walker);
    assert_eq!(calls, 2);
}
#[test]
fn fixed_record_preserves_complete_dynamic_and_inline_event_order_and_exhaustion() {
    let heap = FrozenHeap::new();
    let a = source(&heap, "same.bzl");
    let b = source(&heap, "same.bzl");
    assert_ne!(a.id(), b.id());
    let frame = heap.alloc_any(InlinedFrame {
        span: span(b),
        fun: FrozenValue::new_none(),
    });
    let framed = FrameSpan {
        span: span(a).span,
        inlined_frames: InlinedFrames {
            frames: Some(frame),
        },
    };
    for (allowance, maximum_depth) in [(3, 1), (2, 1), (3, 0)] {
        let mut events = Vec::new();
        let mut visitor = |event| {
            events.push(match event {
                DefDependency::Source(s) if s.file.id() == a.id() => "source:a",
                DefDependency::Source(s) if s.file.id() == b.id() => "source:b",
                DefDependency::Constant(v) if v.is_none() => "helper:none",
                _ => panic!("unexpected dependency"),
            });
            Ok(())
        };
        let mut budget = InspectionBudget {
            remaining_nodes: allowance,
            maximum_depth,
        };
        let mut walker = Walker {
            module: OriginalModule::Missing,
            last_source: None,
            budget: &mut budget,
            visitor: &mut visitor,
        };
        assert_eq!(
            walker.record(&(), framed, 0).is_ok(),
            allowance == 3 && maximum_depth == 1
        );
        drop(walker);
        assert_eq!(
            events,
            if maximum_depth == 0 {
                vec!["source:a"]
            } else if allowance == 3 {
                vec!["source:a", "source:b", "helper:none"]
            } else {
                vec!["source:a", "source:b"]
            }
        );
        assert_eq!(
            budget.remaining_nodes,
            if maximum_depth == 0 { 2 } else { 0 }
        );
    }
    let expression = IrSpanned {
        span: span(a),
        node: ExprCompiled::List(
            (0..2)
                .map(|_| IrSpanned {
                    span: span(a),
                    node: ExprCompiled::Value(FrozenValue::new_none()),
                })
                .collect(),
        ),
    };
    for (allowance, maximum_depth) in [(5, 2), (4, 2), (5, 0)] {
        let mut events = Vec::new();
        let mut visitor = |event| {
            events.push(match event {
                DefDependency::Source(_) => "source",
                DefDependency::Constant(v) if v.is_none() => "value:none",
                _ => panic!("unexpected dependency"),
            });
            Ok(())
        };
        let mut budget = InspectionBudget {
            remaining_nodes: allowance,
            maximum_depth,
        };
        let mut walker = Walker {
            module: OriginalModule::Missing,
            last_source: None,
            budget: &mut budget,
            visitor: &mut visitor,
        };
        assert_eq!(
            walker.expression(&expression, 0).is_ok(),
            allowance == 5 && maximum_depth == 2
        );
        drop(walker);
        assert_eq!(
            events,
            if maximum_depth == 0 {
                vec!["source"]
            } else if allowance == 5 {
                vec!["source", "value:none", "value:none"]
            } else {
                vec!["source", "value:none"]
            }
        );
        assert_eq!(
            budget.remaining_nodes,
            if maximum_depth == 0 { 4 } else { 0 }
        );
    }
}
