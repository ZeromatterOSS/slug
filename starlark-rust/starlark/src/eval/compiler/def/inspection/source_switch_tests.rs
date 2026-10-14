//! Source certificates coalesce by actual map identity; all encounters stay paid.
use super::*;
use crate::codemap::CodeMap;
use crate::codemap::Span;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::compiler::def::inspection::inspect_def;
use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::values::FrozenHeap;

fn evaluate(module: &Module, filename: &str, source: &str) {
    Evaluator::new(module)
        .eval_module(
            AstModule::parse(filename, source.to_owned(), &Dialect::Bazel).unwrap(),
            &Globals::standard(),
        )
        .unwrap();
}
fn trace(module: &Module, name: &str) -> (usize, usize, Vec<String>) {
    let value = module.get(name).unwrap();
    let function = inspect_def(value, Some(module)).unwrap();
    let mut budget = InspectionBudget {
        remaining_nodes: 10_000,
        maximum_depth: 100,
    };
    let mut sources = 0;
    let mut events = Vec::new();
    function
        .code()
        .visit_dependencies(&mut budget, &mut |event| {
            match event {
                DefDependency::Source(_) => sources += 1,
                DefDependency::ModuleSymbol {
                    name,
                    value,
                    access,
                } => {
                    assert_eq!(value.unwrap().unpack_str(), Some("owned"));
                    events.push(format!("module:{name}:{access:?}"));
                }
                DefDependency::Constant(value) => {
                    if let Some(function) = inspect_def(value, None) {
                        events.push(format!("function:{}", function.name()));
                    } else {
                        events.push(format!("constant:{}", value.to_repr()));
                    }
                }
                DefDependency::NativeFunction(native) => {
                    events.push(format!("native:{}", native.name()))
                }
                DefDependency::NestedCode(nested) => {
                    events.push(format!("nested:{}", nested.name()))
                }
                DefDependency::Type(value) => events.push(format!("type:{}", value.get_type())),
            }
            Ok(())
        })
        .unwrap();
    (10_000 - budget.remaining_nodes, sources, events)
}

fn assert_actual_ir_trace_preservation() {
    let repeated = Module::new();
    evaluate(
        &repeated,
        "repeated.bzl",
        r#"
VALUE = "first"
VALUE = "owned"
def callback(x):
    first = x + VALUE
    second = first + VALUE
    def nested(y = x):
        return y + VALUE
    return second + nested(x)
"#,
    );
    let repeated_trace = trace(&repeated, "callback");
    assert_eq!(
        repeated_trace.2,
        [
            "module:VALUE:Read",
            "module:VALUE:Read",
            "nested:nested",
            "module:VALUE:Read"
        ]
    );

    // Same public filename, distinct real module/code maps. This is the same
    // actual frozen helper/inlining route used by the original-module tests.
    let producer = Module::new();
    evaluate(
        &producer,
        "same.bzl",
        "def helper(x):\n    return x + 'suffix'\n",
    );
    let producer = producer.freeze().unwrap();
    let consumer = Module::new();
    let helper = producer
        .get("helper")
        .unwrap()
        .owned_value(consumer.frozen_heap());
    consumer.set("helper", helper);
    drop(producer);
    evaluate(
        &consumer,
        "same.bzl",
        "def callback(x):\n    return helper(x) + helper(x)\n",
    );
    let inline_trace = trace(&consumer, "callback");
    assert_eq!(
        inline_trace.2,
        [
            "function:helper",
            "function:helper",
            "constant:\"suffix\"",
            "function:helper",
            "function:helper",
            "constant:\"suffix\""
        ]
    );
    // Fixed-record admission includes each primary span in its IR record.
    // Standalone/nested sources, inline frames and dependencies remain paid.
    // The prior duplicate-guard walker used 64/30 steps for the same traces.
    assert_eq!((repeated_trace.0, inline_trace.0), (39, 22));
    assert_eq!(repeated_trace.1, 1);
    eprintln!(
        "SOURCE_SWITCH_CANDIDATE repeated_steps={} repeated_sources={} repeated_events={:?} inline_steps={} inline_sources={} inline_events={:?}",
        repeated_trace.0,
        repeated_trace.1,
        repeated_trace.2,
        inline_trace.0,
        inline_trace.1,
        inline_trace.2
    );
}

#[test]
fn source_switch_certificates_preserve_paid_encounters_inline_helpers_and_dependency_order() {
    assert_actual_ir_trace_preservation();
    let heap = FrozenHeap::new();
    let a = heap.alloc_any(CodeMap::new(
        "same.bzl".to_owned(),
        "same bytes\n".to_owned(),
    ));
    let b = heap.alloc_any(CodeMap::new(
        "same.bzl".to_owned(),
        "same bytes\n".to_owned(),
    ));
    assert_ne!(a.id(), b.id());
    let span = |file| FrameSpan::new(FrozenFileSpan::new(file, Span::default()));
    let mut seen = Vec::new();
    let mut budget = InspectionBudget {
        remaining_nodes: 4,
        maximum_depth: 0,
    };
    {
        let mut visitor = |event| {
            let DefDependency::Source(source) = event else {
                panic!("source only")
            };
            seen.push(source.file.id());
            Ok(())
        };
        let mut walker = Walker {
            module: OriginalModule::Missing,
            last_source: None,
            budget: &mut budget,
            visitor: &mut visitor,
        };
        for file in [a, a, b, a] {
            walker.span(span(file), 0).unwrap();
        }
    }
    assert_eq!(budget.remaining_nodes, 0);

    assert_guard_errors(a);
    assert_eq!(seen, [a.id(), b.id(), a.id()]);
}

fn assert_guard_errors(a: crate::values::FrozenRef<'static, CodeMap>) {
    let span = |file| FrameSpan::new(FrozenFileSpan::new(file, Span::default()));
    let mut calls = 0;
    let mut budget = InspectionBudget {
        remaining_nodes: 1,
        maximum_depth: 0,
    };
    {
        let mut visitor = |_| {
            calls += 1;
            Ok(())
        };
        let mut walker = Walker {
            module: OriginalModule::Missing,
            last_source: None,
            budget: &mut budget,
            visitor: &mut visitor,
        };
        walker.span(span(a), 0).unwrap();
        assert!(
            walker
                .span(span(a), 0)
                .unwrap_err()
                .to_string()
                .contains("node budget")
        );
    }
    assert_eq!(calls, 1); // The suppressed duplicate must still fail its budget.
    let mut budget = InspectionBudget {
        remaining_nodes: 2,
        maximum_depth: 0,
    };
    {
        let mut visitor = |_| Ok(());
        let mut walker = Walker {
            module: OriginalModule::Missing,
            last_source: None,
            budget: &mut budget,
            visitor: &mut visitor,
        };
        walker.span(span(a), 0).unwrap();
        assert!(
            walker
                .span(span(a), 1)
                .unwrap_err()
                .to_string()
                .contains("depth budget")
        );
    }
    let mut calls = 0;
    let mut budget = InspectionBudget {
        remaining_nodes: 2,
        maximum_depth: 0,
    };
    {
        let mut visitor = |_| {
            calls += 1;
            Err(crate::Error::new_other(anyhow::anyhow!(
                "source consumer refused"
            )))
        };
        let mut walker = Walker {
            module: OriginalModule::Missing,
            last_source: None,
            budget: &mut budget,
            visitor: &mut visitor,
        };
        assert!(
            walker
                .span(span(a), 0)
                .unwrap_err()
                .to_string()
                .contains("source consumer refused")
        );
    }
    assert_eq!((calls, budget.remaining_nodes), (1, 1));
}
