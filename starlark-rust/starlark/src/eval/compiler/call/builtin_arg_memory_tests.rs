use super::*;
use crate::assert::Assert;
use crate::codemap::CodeMap;
use crate::codemap::Pos;
use crate::codemap::Span;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
use crate::eval::runtime::slots::LocalSlotId;

fn spans(module: &Module) -> [FrameSpan; 8] {
    let file = module.frozen_heap().alloc_any(CodeMap::new(
        "builtin-arg-memory.star".into(),
        "0123456789abcdef".into(),
    ));
    std::array::from_fn(|index| {
        FrameSpan::new(FrozenFileSpan::new(
            file,
            Span::new(Pos::new(index as u32), Pos::new(index as u32 + 1)),
        ))
    })
}

fn local(span: FrameSpan) -> IrSpanned<ExprCompiled> {
    IrSpanned {
        span,
        node: ExprCompiled::Local(LocalSlotId(7)),
    }
}

fn symbols(mut expression: &IrSpanned<ExprCompiled>) -> Vec<(&str, *const u8)> {
    let mut result = Vec::new();
    while let ExprCompiled::Builtin1(Builtin1::Dot(field), object) = &expression.node {
        result.push((field.as_str(), field.as_str().as_ptr()));
        expression = object;
    }
    assert!(matches!(
        expression.node,
        ExprCompiled::Local(LocalSlotId(7))
    ));
    result
}

#[test]
fn builtin_calls_transfer_sole_argument_symbol_buffers_and_preserve_ir() {
    let module = Module::new();
    let globals = Globals::standard();
    let spans = spans(&module);
    let mut eval = Evaluator::new(&module);
    let mut shared_counts = Vec::new();
    for name in ["len", "type"] {
        let mut argument = local(spans[0]);
        for index in 0..3 {
            let field = format!("field_{index}_{}", "owner".repeat(1024));
            argument = IrSpanned {
                span: spans[index + 1],
                node: ExprCompiled::Builtin1(
                    Builtin1::Dot(Symbol::new(&field)),
                    Box::new(argument),
                ),
            };
        }
        let pointers: Vec<_> = symbols(&argument).iter().map(|entry| entry.1).collect();
        let text: Vec<_> = symbols(&argument)
            .iter()
            .map(|entry| entry.0.to_owned())
            .collect();
        let expected_argument = format!("{argument:?}");
        let expected = if name == "len" {
            ExprCompiled::len(spans[6], argument.clone())
        } else {
            ExprCompiled::typ(spans[6], argument.clone())
        };
        let fun = IrSpanned {
            span: spans[4],
            node: ExprCompiled::Value(globals.get_frozen(name).unwrap()),
        };
        let output = CallCompiled::call(
            spans[6],
            spans[7],
            fun,
            ArgsCompiledValue {
                pos_named: vec![argument],
                ..Default::default()
            },
            &mut OptCtx::new(&mut eval, 8),
        );
        assert_eq!(format!("{output:?}"), format!("{expected:?}"));
        let ExprCompiled::Call(call) = output else {
            panic!("expected a dynamic builtin call");
        };
        assert_eq!(call.span, spans[6]);
        assert_eq!(call.node.call_site, spans[6].end_span());
        assert_eq!(call.node.fun.span, spans[6]);
        let argument = call.node.args.one_pos().unwrap();
        assert_eq!(format!("{argument:?}"), expected_argument);
        let actual = symbols(argument);
        assert_eq!(
            actual.iter().map(|entry| entry.0).collect::<Vec<_>>(),
            text.iter().map(String::as_str).collect::<Vec<_>>()
        );
        let shared = actual
            .iter()
            .zip(&pointers)
            .filter(|(actual, original)| actual.1 == **original)
            .count();
        println!(
            "BUILTIN_ARG method={name} symbols={} payload_bytes={} shared={shared}",
            actual.len(),
            text.iter().map(String::len).sum::<usize>()
        );
        shared_counts.push(shared);
    }
    assert_eq!(shared_counts, [3, 3]);
}

#[test]
fn builtin_argument_shape_guards_folding_and_ordinary_calls_are_preserved() {
    let module = Module::new();
    let globals = Globals::standard();
    let spans = spans(&module);
    let mut eval = Evaluator::new(&module);
    for name in ["len", "type"] {
        let fun = || IrSpanned {
            span: spans[4],
            node: ExprCompiled::Value(globals.get_frozen(name).unwrap()),
        };
        for shape in 0..6 {
            let mut args = ArgsCompiledValue::default();
            match shape {
                0 => {}
                1 => args.pos_named = vec![local(spans[0]), local(spans[1])],
                2 => {
                    args.pos_named = vec![local(spans[0])];
                    args.names = vec![(Symbol::new("x"), module.frozen_heap().alloc_str("x"))];
                }
                3 => {
                    args.pos_named = vec![local(spans[0])];
                    args.args = Some(local(spans[1]));
                }
                4 => {
                    args.pos_named = vec![local(spans[0])];
                    args.kwargs = Some(local(spans[1]));
                }
                5 => {
                    args.args = Some(local(spans[0]));
                    args.kwargs = Some(local(spans[1]));
                }
                _ => unreachable!(),
            }
            assert!(args.one_pos().is_none());
            let expected_fun = format!("{:?}", fun());
            let expected_args = format!("{args:?}");
            let output = CallCompiled::call(
                spans[6],
                spans[7],
                fun(),
                args,
                &mut OptCtx::new(&mut eval, 8),
            );
            let ExprCompiled::Call(call) = output else {
                panic!("{name} shape {shape} must retain normal call");
            };
            assert_eq!(call.span, spans[6]);
            assert_eq!(call.node.call_site, spans[7]);
            assert_eq!(format!("{:?}", call.node.fun), expected_fun);
            assert_eq!(format!("{:?}", call.node.args), expected_args);
        }
        let value = module.frozen_heap().alloc_str("abc").to_frozen_value();
        let output = CallCompiled::call(
            spans[6],
            spans[7],
            fun(),
            ArgsCompiledValue {
                pos_named: vec![IrSpanned {
                    span: spans[0],
                    node: ExprCompiled::Value(value),
                }],
                ..Default::default()
            },
            &mut OptCtx::new(&mut eval, 8),
        );
        let ExprCompiled::Value(value) = output else {
            panic!("expected constant folding");
        };
        if name == "len" {
            assert_eq!(value.to_value().unpack_i32(), Some(3));
        } else {
            assert_eq!(value.unpack_str(), Some("string"));
        }
    }
    let fun = local(spans[4]);
    let args = ArgsCompiledValue {
        pos_named: vec![local(spans[0])],
        ..Default::default()
    };
    let expected_fun = format!("{fun:?}");
    let expected_args = format!("{args:?}");
    let output = CallCompiled::call(
        spans[6],
        spans[7],
        fun,
        args,
        &mut OptCtx::new(&mut eval, 8),
    );
    let ExprCompiled::Call(call) = output else {
        panic!("expected ordinary call");
    };
    assert_eq!(call.span, spans[6]);
    assert_eq!(call.node.call_site, spans[7]);
    assert_eq!(format!("{:?}", call.node.fun), expected_fun);
    assert_eq!(format!("{:?}", call.node.args), expected_args);
}

#[test]
fn builtin_calls_preserve_effects_errors_and_retained_frozen_functions() {
    let source = String::from(
        r#"
def seen(log, value):
    log.append("value")
    return value
def length(log, value):
    return len(seen(log, value).outer.inner)
def kind(log, value):
    return type(seen(log, value).outer.inner)
def starred_length(log, values):
    return len(*seen(log, values))
def starred_kind(log, values):
    return type(*seen(log, values))
def named_length(log, values):
    return len(**seen(log, values))
def named_kind(log, values):
    return type(**seen(log, values))
log = []
assert_eq(length(log, struct(outer=struct(inner="abc"))), 3)
assert_eq(kind(log, struct(outer=struct(inner="abc"))), "string")
assert_eq(log, ["value", "value"])
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions = [
        "length",
        "kind",
        "starred_length",
        "starred_kind",
        "named_length",
        "named_kind",
    ]
    .map(|name| (name, module.get(name).unwrap()));
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let mut assertion = Assert::new();
    assertion.globals_add(|builder| {
        for (name, function) in functions {
            builder.set(name, function);
        }
    });
    assertion.pass(r#"
log = []
RESULT = []
for value, size, name in [("", 0, "string"), ("é世界", 3, "string"), ([1,2], 2, "list"), ({"é": 1}, 1, "dict")]:
    receiver = struct(outer=struct(inner=value))
    RESULT.append(length(log, receiver))
    RESULT.append(kind(log, receiver))
    assert_eq(RESULT[-2:], [size, name])
    assert_eq(starred_length(log, [value]), size)
    assert_eq(starred_kind(log, [value]), name)
receiver = None
value = None
garbage_collect()
assert_eq(RESULT, [0, "string", 3, "string", 2, "list", 1, "dict"])
assert_eq(log, ["value"] * 16)
assert_eq(kind([], struct(outer=struct(inner=True))), "bool")
assert_eq(kind([], struct(outer=struct(inner=None))), "NoneType")
garbage_collect()
"#);
    // Disable static type checking so arity/shape failures exercise real invocation.
    assertion.disable_static_typechecking();
    let errors: Vec<_> = [
        ("length([], struct())", "has no attribute `outer`"),
        (
            "kind([], struct(outer=struct()))",
            "has no attribute `inner`",
        ),
        (
            "length([], struct(outer=struct(inner=1)))",
            "Operation `len()` not supported on type `int`",
        ),
        ("starred_length([], [])", ""),
        ("starred_kind([], [1, 2])", ""),
        ("named_length([], {'x': [1]})", ""),
        ("named_kind([], {'x': [1]})", ""),
    ]
    .into_iter()
    .map(|(program, message)| format!("{:#}", assertion.fails(program, &[message])))
    .collect();
    println!(
        "BUILTIN_ARG_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}
