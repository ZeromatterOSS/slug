use super::*;
use crate::assert::Assert;
use crate::environment::Globals;
use crate::environment::Module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::syntax::StringEncoding;

#[test]
fn empty_segments_reuse_mutable_and_frozen_inputs() {
    let text = "é".repeat(32768);
    let frozen_heap = FrozenHeap::new();
    let frozen = frozen_heap.alloc_str(&text);
    Heap::temp(|heap| {
        let input = heap.alloc_str(&text);
        let tuple = heap.alloc((input,));
        let mut results = Vec::new();
        for (name, arg, expected) in [
            ("dot", input.to_value(), input.to_value()),
            ("percent", input.to_value(), input.to_value()),
            ("tuple", tuple, input.to_value()),
            ("frozen", frozen.to_value(), frozen.to_value()),
        ] {
            let before = heap.allocated_bytes();
            let output = if name == "dot" {
                format_one("", arg, "", heap)
            } else {
                percent_s_one("", arg, "", heap).unwrap()
            };
            assert_eq!(output.as_str(), text);
            let growth = heap.allocated_bytes() - before;
            let shares = output.to_value().ptr_eq(expected);
            println!(
                "EMPTY_FORMAT_HELPER {name} bytes={} arena_growth={growth} shares={shares}",
                text.len()
            );
            results.push((name, growth, shares));
        }
        for (name, growth, shares) in results {
            assert!(shares, "{name}");
            assert_eq!(growth, 0, "{name}");
        }
    });
}

#[test]
fn empty_segment_compiler_constants_skip_mutable_roundtrip() {
    let text = "é".repeat(32768);
    let foreign = FrozenHeap::new();
    let input = foreign.alloc_str(&text);
    let module = Module::new();
    let mut eval = Evaluator::new(&module);
    let empty = module.frozen_heap().alloc_str("");
    let mut results = Vec::new();
    for percent in [false, true] {
        let before = module.heap().allocated_bytes();
        let arg = IrSpanned {
            node: ExprCompiled::Value(input.to_frozen_value()),
            span: Default::default(),
        };
        let mut ctx = OptCtx::new(&mut eval, 0);
        let result = if percent {
            ExprCompiled::percent_s_one(empty, arg, empty, &mut ctx)
        } else {
            ExprCompiled::format_one(empty, arg, empty, &mut ctx)
        };
        let ExprCompiled::Value(value) = result else {
            panic!("constant must remain folded");
        };
        assert_eq!(value.unpack_str(), Some(text.as_str()));
        let own = module.frozen_heap().alloc_str(&text);
        assert!(value.to_value().ptr_eq(own.to_value()));
        assert!(!value.to_value().ptr_eq(input.to_value()));
        let growth = module.heap().allocated_bytes() - before;
        println!(
            "EMPTY_FORMAT_COMPILER percent={percent} bytes={} arena_growth={growth}",
            text.len()
        );
        results.push(growth);
    }
    drop(foreign);
    let own = module.frozen_heap().alloc_str(&text);
    assert_eq!(own.as_str(), text);
    assert_eq!(results, [0, 0]);
}

#[test]
fn empty_segment_semantics_and_retained_compiled_functions() {
    Heap::temp(|heap| {
        for text in ["", "x", "é", "\0世界"] {
            let input = heap.alloc_str(text).to_value();
            assert_eq!(format_one("", input, "", heap).as_str(), text);
            assert_eq!(percent_s_one("", input, "", heap).unwrap().as_str(), text);
            assert_eq!(
                format_one("<", input, ">", heap).as_str(),
                format!("<{text}>")
            );
            assert_eq!(
                percent_s_one("<", input, ">", heap).unwrap().as_str(),
                format!("<{text}>")
            );
        }
        let number = heap.alloc(7);
        assert_eq!(format_one("", number, "", heap).as_str(), "7");
        assert_eq!(percent_s_one("", number, "", heap).unwrap().as_str(), "7");
        assert_eq!(format_one("", heap.alloc((7,)), "", heap).as_str(), "(7,)");
        assert_eq!(
            percent_s_one(
                "",
                heap.alloc(crate::values::tuple::AllocTuple::EMPTY),
                "",
                heap
            )
            .unwrap_err()
            .to_string(),
            "Not enough arguments for format string"
        );
        assert_eq!(
            percent_s_one("", heap.alloc((1, 2)), "", heap)
                .unwrap_err()
                .to_string(),
            "Too many arguments for format string"
        );
    });
    let mut a = Assert::new();
    a.module("payload.bzl", "TOKEN = 'é世界-token'");
    let source = String::from(
        r#"
load("payload.bzl", "TOKEN")
def plain(x): return "{}".format(x)
def percent(x): return "%s" % x
def const_plain(): return "{}".format(TOKEN)
def const_percent(): return "%s" % TOKEN
def decorated(x): return "<{}>".format(x)
s = "é世界-runtime" + str(7)
RESULT = [plain(s), percent(s), percent((s,))]
s = None
garbage_collect()
assert_eq(RESULT, ["é世界-runtime7"] * 3)
assert_eq(const_plain(), TOKEN)
assert_eq(const_percent(), TOKEN)
"#,
    );
    let module = a.pass_module(&source);
    let functions = [
        "plain",
        "percent",
        "const_plain",
        "const_percent",
        "decorated",
    ]
    .map(|name| (name, module.get(name).unwrap()));
    let held = module.get("RESULT").unwrap();
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    drop(a);
    let mut caller = Assert::new();
    caller.globals_add(|builder| {
        builder.set("HELD", held);
        for (name, function) in functions {
            builder.set(name, function);
        }
    });
    caller.pass(
        r#"
assert_eq(HELD, ["é世界-runtime7"] * 3)
RESULT = [plain("X"), percent(("X",)), const_plain(), const_percent(), decorated(7)]
garbage_collect()
assert_eq(RESULT, ["X", "X", "é世界-token", "é世界-token", "<7>"])
assert_eq(plain((7,)), "(7,)")
assert_eq(percent(7), "7")
"#,
    );
    caller.fails("percent(())", &["Not enough arguments for format string"]);
    caller.fails("percent((1, 2))", &["Too many arguments for format string"]);
}

#[test]
fn empty_segment_compiled_functions_preserve_bazel_internal_carriers() {
    let functions = {
        let ast = AstModule::parse_with_string_encoding(
            "empty-format-memory.bzl",
            "def plain():\n    return '{}'.format('é界')\ndef percent(x):\n    return '%s' % ('é' + x)\n".to_owned(),
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
    assert_eq!(
        eval.eval_function(functions[0].value(), &[], &[])
            .unwrap()
            .unpack_str(),
        Some("\u{c3}\u{a9}\u{e7}\u{95}\u{8c}")
    );
    assert_eq!(
        eval.eval_function(functions[1].value(), &[caller.heap().alloc("X")], &[])
            .unwrap()
            .unpack_str(),
        Some("\u{c3}\u{a9}X")
    );
}
