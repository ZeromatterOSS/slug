use super::*;
use crate::assert::Assert;
use crate::codemap::CodeMap;
use crate::codemap::Pos;
use crate::codemap::Span;
use crate::environment::Globals;
use crate::environment::Module;

fn spans(module: &Module) -> [FrameSpan; 8] {
    let file = module.frozen_heap().alloc_any(CodeMap::new(
        "type-equality-memory.star".into(),
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

fn dot(span: FrameSpan, object: IrSpanned<ExprCompiled>, name: &str) -> IrSpanned<ExprCompiled> {
    IrSpanned {
        span,
        node: ExprCompiled::Builtin1(Builtin1::Dot(Symbol::new(name)), Box::new(object)),
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
fn type_equality_transfers_symbols_in_both_orders_and_preserves_ir_spans() {
    let module = Module::new();
    let globals = Globals::standard();
    let spans = spans(&module);
    let mut eval = Evaluator::new(&module);
    let name = module.frozen_heap().alloc_str("list");
    let mut shared_counts = Vec::new();
    for reversed in [false, true] {
        let mut argument = local(spans[0]);
        for index in 0..3 {
            argument = dot(
                spans[index + 1],
                argument,
                &format!("field_{index}_{}", "owner".repeat(1024)),
            );
        }
        let pointers: Vec<_> = symbols(&argument).iter().map(|entry| entry.1).collect();
        let text: Vec<_> = symbols(&argument)
            .iter()
            .map(|entry| entry.0.to_owned())
            .collect();
        let expected_argument = format!("{argument:?}");
        let expected_node = ExprCompiled::type_is(argument.clone(), name);
        let call = IrSpanned {
            span: spans[5],
            node: CallCompiled::call(
                spans[5],
                spans[4],
                IrSpanned {
                    span: spans[4],
                    node: ExprCompiled::Value(globals.get_frozen("type").unwrap()),
                },
                ArgsCompiledValue {
                    pos_named: vec![argument],
                    ..Default::default()
                },
                &mut OptCtx::new(&mut eval, 8),
            ),
        };
        assert!(call.as_type().is_some());
        let comparison = IrSpanned {
            span: spans[6],
            node: ExprCompiled::Value(name.to_frozen_value()),
        };
        let expected_span = if reversed {
            comparison.span.merge(&call.span)
        } else {
            call.span.merge(&comparison.span)
        };
        let output = if reversed {
            ExprCompiled::equals(comparison, call)
        } else {
            ExprCompiled::equals(call, comparison)
        };
        assert_eq!(output.span, expected_span);
        assert_eq!(format!("{:?}", output.node), format!("{expected_node:?}"));
        let ExprCompiled::Builtin1(Builtin1::TypeIs(actual_name), argument) = output.node else {
            panic!("expected type equality folding");
        };
        assert!(actual_name.to_value().ptr_eq(name.to_value()));
        assert_eq!(format!("{argument:?}"), expected_argument);
        let actual = symbols(&argument);
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
            "TYPE_EQUALITY reversed={reversed} symbols={} payload_bytes={} shared={shared}",
            actual.len(),
            text.iter().map(String::len).sum::<usize>()
        );
        shared_counts.push(shared);
    }
    assert_eq!(shared_counts, [3, 3]);
}

fn operand_pointers(expression: &IrSpanned<ExprCompiled>) -> Vec<*const u8> {
    match &expression.node {
        ExprCompiled::Call(call) => call
            .node
            .args
            .arg_exprs()
            .flat_map(|e| symbols(e).into_iter().map(|entry| entry.1))
            .chain(
                call.node
                    .args
                    .names
                    .iter()
                    .map(|entry| entry.0.as_str().as_ptr()),
            )
            .collect(),
        ExprCompiled::Builtin1(Builtin1::Dot(_), _) => {
            symbols(expression).iter().map(|entry| entry.1).collect()
        }
        _ => Vec::new(),
    }
}

#[test]
fn type_equality_rejections_preserve_operands_pointers_and_constant_fallbacks() {
    let module = Module::new();
    let globals = Globals::standard();
    let spans = spans(&module);
    let comparison = || IrSpanned {
        span: spans[6],
        node: ExprCompiled::Value(module.frozen_heap().alloc_str("list").to_frozen_value()),
    };
    for shape in 0..10 {
        let arg = || dot(spans[1], local(spans[0]), "field");
        let mut args = ArgsCompiledValue::default();
        match shape {
            0 => {}
            1 => args.pos_named = vec![arg(), arg()],
            2 => {
                args.pos_named = vec![arg()];
                args.names = vec![(Symbol::new("x"), module.frozen_heap().alloc_str("x"))];
            }
            3 => {
                args.pos_named = vec![arg()];
                args.args = Some(arg());
            }
            4 => {
                args.pos_named = vec![arg()];
                args.kwargs = Some(arg());
            }
            5 => {
                args.args = Some(arg());
                args.kwargs = Some(arg());
            }
            _ => args.pos_named = vec![arg()],
        }
        let mut left = IrSpanned {
            span: spans[5],
            node: ExprCompiled::Call(Box::new(IrSpanned {
                span: spans[5],
                node: CallCompiled {
                    call_site: spans[4],
                    fun: IrSpanned {
                        span: spans[4],
                        node: ExprCompiled::Value(
                            globals
                                .get_frozen(if shape == 6 { "len" } else { "type" })
                                .unwrap(),
                        ),
                    },
                    args,
                },
            })),
        };
        if shape == 7 {
            left = arg();
        }
        let right = match shape {
            8 => local(spans[6]),
            9 => IrSpanned {
                span: spans[6],
                node: ExprCompiled::Value(FrozenValue::new_bool(false)),
            },
            _ => comparison(),
        };
        let expected = (format!("{left:?}"), format!("{right:?}"));
        let pointers = (operand_pointers(&left), operand_pointers(&right));
        let Err((left, right)) = try_eval_type_is(left, right) else {
            panic!("shape {shape} must reject");
        };
        assert_eq!((format!("{left:?}"), format!("{right:?}")), expected);
        assert_eq!(
            (operand_pointers(&left), operand_pointers(&right)),
            pointers
        );
        let expected_span = left.span.merge(&right.span);
        let output = ExprCompiled::equals(left, right);
        assert_eq!(output.span, expected_span);
        let ExprCompiled::Builtin2(Builtin2::Equals, operands) = output.node else {
            panic!("expected ordinary equality fallback");
        };
        assert_eq!(
            (format!("{:?}", operands.0), format!("{:?}", operands.1)),
            expected
        );
        assert_eq!(
            (operand_pointers(&operands.0), operand_pointers(&operands.1)),
            pointers
        );
    }
    let value = module.frozen_heap().alloc_str("é").to_frozen_value();
    let output = ExprCompiled::equals(
        IrSpanned {
            span: spans[5],
            node: ExprCompiled::Value(value),
        },
        IrSpanned {
            span: spans[6],
            node: ExprCompiled::Value(value),
        },
    );
    assert_eq!(output.span, spans[5].merge(&spans[6]));
    let ExprCompiled::Value(value) = output.node else {
        panic!("expected constant folding");
    };
    assert_eq!(value.unpack_bool(), Some(true));
}

#[test]
fn type_equality_preserves_effects_errors_and_retained_frozen_functions() {
    let source = String::from(
        r#"
def seen(log, tag, value):
    log.append(tag)
    return value
def direct(log, value):
    return type(seen(log, "direct", value).outer.inner) == "list"
def reverse(log, value):
    return "list" == type(seen(log, "reverse", value).outer.inner)
def negated(log, value):
    return type(seen(log, "negated", value).outer.inner) != "list"
def star(log, values):
    return type(*seen(log, "star", values)) == "list"
def named(log, values):
    return type(**seen(log, "named", values)) == "list"
def pair(log, value, name):
    return type(seen(log, "left", value).outer.inner) == seen(log, "right", name)
log = []
assert_eq(direct(log, struct(outer=struct(inner=[]))), True)
assert_eq(reverse(log, struct(outer=struct(inner={}))), False)
assert_eq(log, ["direct", "reverse"])
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions = ["direct", "reverse", "negated", "star", "named", "pair"]
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
    assertion.pass(
        r#"
log = []
RESULT = []
for value, expected in [([], True), ([1], True), ({}, False), ("é", False), (1, False)]:
    receiver = struct(outer=struct(inner=value))
    RESULT.append(direct(log, receiver))
    assert_eq(reverse(log, receiver), expected)
    assert_eq(negated(log, receiver), not expected)
    assert_eq(star(log, [value]), expected)
receiver = None
value = None
garbage_collect()
assert_eq(RESULT, [True, True, False, False, False])
assert_eq(log, ["direct", "reverse", "negated", "star"] * 5)
log = []
assert_eq(pair(log, struct(outer=struct(inner="é世界")), "string"), True)
assert_eq(log, ["left", "right"])
garbage_collect()
"#,
    );
    assertion.disable_static_typechecking();
    let errors: Vec<_> = [
        "direct([], struct())",
        "reverse([], struct(outer=struct()))",
        "star([], [])",
        "star([], [[], {}])",
        "named([], {'x': []})",
    ]
    .into_iter()
    .map(|program| format!("{:#}", assertion.fails(program, &[])))
    .collect();
    println!(
        "TYPE_EQUALITY_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}
