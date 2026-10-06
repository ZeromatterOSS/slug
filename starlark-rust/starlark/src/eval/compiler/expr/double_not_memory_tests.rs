use super::*;
use crate::assert::Assert;
use crate::codemap::CodeMap;
use crate::codemap::Pos;
use crate::codemap::Span;
use crate::environment::Module;

fn spans(module: &Module) -> [FrameSpan; 8] {
    let file = module.frozen_heap().alloc_any(CodeMap::new(
        "double-not-memory.star".into(),
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
fn double_not_transfers_symbol_buffers_and_preserves_expression_spans() {
    let module = Module::new();
    let spans = spans(&module);
    let mut receiver = local(spans[0]);
    for index in 0..3 {
        let name = format!("field_{index}_{}", "owner".repeat(1024));
        receiver = IrSpanned {
            span: spans[index + 1],
            node: ExprCompiled::Builtin1(Builtin1::Dot(Symbol::new(&name)), Box::new(receiver)),
        };
    }
    let pointers: Vec<_> = symbols(&receiver).iter().map(|entry| entry.1).collect();
    let names: Vec<_> = symbols(&receiver)
        .iter()
        .map(|entry| entry.0.to_owned())
        .collect();
    let expression = IrSpanned {
        span: spans[4],
        node: ExprCompiled::Builtin2(Builtin2::Equals, Box::new((receiver, local(spans[5])))),
    };
    let expected = format!("{expression:?}");
    let output = ExprCompiled::not(
        spans[7],
        IrSpanned {
            span: spans[6],
            node: ExprCompiled::Builtin1(Builtin1::Not, Box::new(expression)),
        },
    );
    assert_eq!(format!("{output:?}"), expected);
    assert_eq!(output.span, spans[4]);
    let ExprCompiled::Builtin2(Builtin2::Equals, operands) = &output.node else {
        panic!("expected the original equality expression");
    };
    let actual = symbols(&operands.0);
    assert_eq!(
        actual.iter().map(|entry| entry.0).collect::<Vec<_>>(),
        names.iter().map(String::as_str).collect::<Vec<_>>()
    );
    let shared = actual
        .iter()
        .zip(&pointers)
        .filter(|(actual, original)| actual.1 == **original)
        .count();
    println!(
        "DOUBLE_NOT symbols={} payload_bytes={} shared={shared}",
        actual.len(),
        names.iter().map(String::len).sum::<usize>()
    );
    assert_eq!(shared, 3);
}

#[test]
fn boolean_guard_constant_truthiness_and_fallback_spans_are_preserved() {
    let module = Module::new();
    let spans = spans(&module);
    let operand = || local(spans[0]);
    let mut bool_nodes = vec![
        ExprCompiled::Value(FrozenValue::new_bool(false)),
        ExprCompiled::Builtin1(Builtin1::Not, Box::new(operand())),
        ExprCompiled::Builtin1(
            Builtin1::TypeIs(module.frozen_heap().alloc_str("string")),
            Box::new(operand()),
        ),
    ];
    for op in [
        Builtin2::In,
        Builtin2::Equals,
        Builtin2::Compare(CompareOp::Less),
        Builtin2::Compare(CompareOp::Greater),
        Builtin2::Compare(CompareOp::LessOrEqual),
        Builtin2::Compare(CompareOp::GreaterOrEqual),
    ] {
        bool_nodes.push(ExprCompiled::Builtin2(op, Box::new((operand(), operand()))));
    }
    for node in bool_nodes {
        let expression = IrSpanned {
            span: spans[1],
            node,
        };
        let expected = format!("{expression:?}");
        let output = ExprCompiled::not(
            spans[3],
            IrSpanned {
                span: spans[2],
                node: ExprCompiled::Builtin1(Builtin1::Not, Box::new(expression)),
            },
        );
        assert_eq!(format!("{output:?}"), expected);
        assert_eq!(output.span, spans[1]);
    }
    for node in [
        ExprCompiled::Local(LocalSlotId(7)),
        ExprCompiled::Builtin1(Builtin1::Dot(Symbol::new("field")), Box::new(operand())),
        ExprCompiled::List(vec![operand()]),
        ExprCompiled::LogicalBinOp(ExprLogicalBinOp::And, Box::new((operand(), operand()))),
    ] {
        let inner = IrSpanned {
            span: spans[1],
            node,
        };
        let expression = IrSpanned {
            span: spans[2],
            node: ExprCompiled::Builtin1(Builtin1::Not, Box::new(inner)),
        };
        let expected = format!("{expression:?}");
        let output = ExprCompiled::not(spans[3], expression);
        assert_eq!(output.span, spans[3]);
        let ExprCompiled::Builtin1(Builtin1::Not, original) = output.node else {
            panic!("nonboolean expressions must retain both negations");
        };
        assert_eq!(format!("{original:?}"), expected);
    }
    for (value, expected) in [
        (FrozenValue::new_bool(false), true),
        (FrozenValue::new_bool(true), false),
        (module.frozen_heap().alloc_str("").to_frozen_value(), true),
        (module.frozen_heap().alloc_str("é").to_frozen_value(), false),
    ] {
        let output = ExprCompiled::not(
            spans[3],
            IrSpanned {
                span: spans[0],
                node: ExprCompiled::Value(value),
            },
        );
        assert_eq!(output.span, spans[3]);
        let ExprCompiled::Value(value) = output.node else {
            panic!("expected constant folding");
        };
        assert_eq!(value.unpack_bool(), Some(expected));
    }
}

#[test]
fn double_not_evaluation_preserves_effects_errors_and_frozen_functions() {
    let source = String::from(
        r#"
def seen(log, tag, value):
    log.append(tag)
    return value
def folded(log, left, right):
    return not not (seen(log, "left", left).outer.inner == seen(log, "right", right).outer.inner)
def nested(log, value):
    return not not not not (seen(log, "nested", value) == 1)
def general(log, value):
    return not not seen(log, "value", value)
log = []
assert_eq(folded(log, struct(outer=struct(inner="é")), struct(outer=struct(inner="é"))), True)
assert_eq(log, ["left", "right"])
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions = ["folded", "nested", "general"].map(|name| (name, module.get(name).unwrap()));
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
left = struct(outer=struct(inner="é"))
right = struct(outer=struct(inner="世界"))
RESULT = [folded(log, left, left), folded(log, left, right), nested(log, 1), nested(log, 0)]
left = None
right = None
garbage_collect()
assert_eq(RESULT, [True, False, True, False])
assert_eq(log, ["left", "right", "left", "right", "nested", "nested"])
for value, expected in [(False, False), (True, True), (0, False), (2, True), ("", False), ("é", True), ([], False), ([1], True)]:
    log = []
    assert_eq(general(log, value), expected)
    assert_eq(log, ["value"])
garbage_collect()
"#);
    let errors: Vec<_> = [
        (
            "folded([], struct(), struct(outer=struct(inner=1)))",
            "has no attribute `outer`",
        ),
        (
            "folded([], struct(outer=struct(inner=1)), struct(outer=struct()))",
            "has no attribute `inner`",
        ),
    ]
    .into_iter()
    .map(|(program, message)| format!("{:#}", assertion.fails(program, &[message])))
    .collect();
    println!(
        "DOUBLE_NOT_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}
