use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use starlark_derive::starlark_value;

use super::format_one;
use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::assert::Assert;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::starlark_simple_value;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::syntax::StringEncoding;
use crate::values::Heap;
use crate::values::NoSerialize;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::string::interpolation::percent_s_one;

type RenderTrace = Arc<Mutex<Vec<(String, usize)>>>;

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
struct ScratchObserver {
    #[allocative(skip)]
    trace: RenderTrace,
}

impl fmt::Display for ScratchObserver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("repr-form")
    }
}

starlark_simple_value!(ScratchObserver);

#[starlark_value(type = "scratch_observer")]
impl<'v> StarlarkValue<'v> for ScratchObserver {
    fn collect_str(&self, collector: &mut String) {
        self.trace
            .lock()
            .unwrap()
            .push((collector.clone(), collector.capacity()));
        collector.push_str("str-form");
    }
}

#[test]
fn suffix_is_excluded_from_initial_renderer_scratch() {
    let before = "é:";
    let after = "x".repeat(1024 * 1024);
    Heap::temp(|heap| {
        let mut capacities = Vec::new();
        for percent in [false, true] {
            let trace = RenderTrace::default();
            let arg = heap.alloc_simple(ScratchObserver {
                trace: trace.clone(),
            });
            let output = if percent {
                percent_s_one(before, heap.alloc((arg,)), &after, heap).unwrap()
            } else {
                format_one(before, arg, &after, heap)
            };
            assert_eq!(output.as_str(), format!("{before}str-form{after}"));
            let observed = trace.lock().unwrap();
            assert_eq!(observed.len(), 1);
            assert_eq!(observed[0].0, before);
            let capacity = observed[0].1;
            println!(
                "FORMAT_SUFFIX percent={percent} prefix_bytes={} suffix_bytes={} initial_capacity={capacity} output_bytes={}",
                before.len(),
                after.len(),
                output.as_str().len()
            );
            capacities.push(capacity);
        }
        assert_eq!(capacities, [before.len() + 10; 2]);
    });
}

#[test]
fn renderer_context_and_retained_compiled_results_are_preserved() {
    Heap::temp(|heap| {
        let trace = RenderTrace::default();
        let custom = heap.alloc_simple(ScratchObserver {
            trace: trace.clone(),
        });
        for (before, after) in [("", ""), ("é[", "\0世界]"), ("prefix:", "tail")] {
            assert_eq!(
                format_one(before, custom, after, heap).as_str(),
                format!("{before}str-form{after}")
            );
            assert_eq!(
                percent_s_one(before, custom, after, heap).unwrap().as_str(),
                format!("{before}str-form{after}")
            );
        }
        assert_eq!(
            trace
                .lock()
                .unwrap()
                .iter()
                .map(|x| x.0.as_str())
                .collect::<Vec<_>>(),
            ["", "", "é[", "é[", "prefix:", "prefix:"]
        );
        for (arg, expected) in [
            (heap.alloc(7), "7"),
            (heap.alloc(true), "True"),
            (Value::new_none(), "None"),
            (heap.alloc((7,)), "(7,)"),
        ] {
            assert_eq!(
                format_one("é:", arg, ":世界", heap).as_str(),
                format!("é:{expected}:世界")
            );
        }
        let input = heap.alloc_str("é\0世界");
        assert!(
            format_one("", input.to_value(), "", heap)
                .to_value()
                .ptr_eq(input.to_value())
        );
        assert_eq!(
            format_one("<", input.to_value(), ">", heap).as_str(),
            "<é\0世界>"
        );
    });
    let source = String::from(
        r#"
def plain(x): return "é:{}:世界".format(x)
def percent(x): return "é:%s:世界" % x
def const_plain(): return "é:{}:世界".format(7)
def const_percent(): return "é:%s:世界" % 7
RESULT = [plain(7), percent((7,)), const_plain(), const_percent()]
garbage_collect()
assert_eq(RESULT, ["é:7:世界"] * 4)
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions = ["plain", "percent", "const_plain", "const_percent"]
        .map(|name| (name, module.get(name).unwrap()));
    let held = module.get("RESULT").unwrap();
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let mut caller = Assert::new();
    caller.globals_add(|builder| {
        builder.set("HELD", held);
        for (name, function) in functions {
            builder.set(name, function);
        }
    });
    caller.pass(
        r#"
assert_eq(HELD, ["é:7:世界"] * 4)
RESULT = [plain((7,)), percent((7,)), const_plain(), const_percent()]
garbage_collect()
assert_eq(RESULT, ["é:(7,):世界", "é:7:世界", "é:7:世界", "é:7:世界"])
assert_eq(plain("X"), "é:X:世界")
assert_eq(percent("X"), "é:X:世界")
"#,
    );
    caller.fails("percent(())", &["Not enough arguments for format string"]);
    caller.fails("percent((1, 2))", &["Too many arguments for format string"]);
}

#[test]
fn renderer_suffix_preserves_compiled_bazel_internal_carriers() {
    let functions = {
        let ast = AstModule::parse_with_string_encoding(
            "format-suffix-memory.bzl",
            "def plain(x):\n    return 'é:{}界'.format(x)\ndef percent(x):\n    return 'é:%s界' % x\n".to_owned(),
            &Dialect::Bazel,
            StringEncoding::BazelInternal,
        ).unwrap();
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(ast, &Globals::standard())
            .unwrap();
        let frozen = module.freeze().unwrap();
        [frozen.get("plain").unwrap(), frozen.get("percent").unwrap()]
    };
    let caller = Module::new();
    let mut eval = Evaluator::new(&caller);
    for function in &functions {
        assert_eq!(
            eval.eval_function(function.value(), &[caller.heap().alloc(7)], &[])
                .unwrap()
                .unpack_str(),
            Some("\u{c3}\u{a9}:7\u{e7}\u{95}\u{8c}")
        );
    }
}
