use super::*;
use crate::assert::Assert;
use crate::codemap::CodeMap;
use crate::codemap::Pos;
use crate::codemap::Span;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
use crate::eval::runtime::slots::LocalSlotId;

fn spans(module: &Module) -> [FrameSpan; 8] {
    let file = module.frozen_heap().alloc_any(CodeMap::new(
        "terminal-arg-memory.star".into(),
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

fn frozen_function(module: &Module, name: &str) -> ExprCompiled {
    let library = Assert::new()
        .pass_module("def is_list(x): return type(x) == type([])\ndef ordinary(x): return x\n");
    let function = library.get(name).unwrap();
    ExprCompiled::Value(
        function
            .owned_value(module.frozen_heap())
            .unpack_frozen()
            .unwrap(),
    )
}

fn bound_method(module: &Module, text: &str, method: &str, ctx: &mut OptCtx) -> ExprCompiled {
    let value = module.frozen_heap().alloc_str(text).to_frozen_value();
    ExprCompiled::Value(
        ExprCompiled::compile_time_getattr(value, &Symbol::new(method), ctx).unwrap(),
    )
}

#[test]
fn terminal_call_optimizations_transfer_symbols_and_preserve_ir_and_frozen_parts() {
    let module = Module::new();
    let spans = spans(&module);
    let mut eval = Evaluator::new(&module);
    let mut ctx = OptCtx::new(&mut eval, 8);
    let mut shared_counts = Vec::new();
    for route in ["type_is", "format"] {
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
        let node = if route == "type_is" {
            frozen_function(&module, "is_list")
        } else {
            bound_method(&module, "pre{}post", "format", &mut ctx)
        };
        let expected = if route == "type_is" {
            let function = node.as_frozen_def().unwrap();
            let Some(InlineDefBody::ReturnTypeIs(t)) = &function.def_info.inline_def_body else {
                panic!("expected real frozen type-is inline metadata");
            };
            ExprCompiled::type_is(argument.clone(), *t)
        } else {
            let before = ctx.frozen_heap().alloc_str_intern("pre");
            let after = ctx.frozen_heap().alloc_str_intern("post");
            ExprCompiled::format_one(before, argument.clone(), after, &mut ctx)
        };
        let output = CallCompiled::call(
            spans[6],
            spans[7],
            IrSpanned {
                span: spans[4],
                node,
            },
            ArgsCompiledValue {
                pos_named: vec![argument],
                ..Default::default()
            },
            &mut ctx,
        );
        assert_eq!(format!("{output:?}"), format!("{expected:?}"));
        let ExprCompiled::Builtin1(op, argument) = &output else {
            panic!("expected terminal unary optimization");
        };
        match op {
            Builtin1::TypeIs(t) => assert_eq!(t.as_str(), "list"),
            Builtin1::FormatOne(before, after) => {
                assert_eq!(before.as_str(), "pre");
                assert_eq!(after.as_str(), "post");
                assert!(
                    before
                        .to_value()
                        .ptr_eq(ctx.frozen_heap().alloc_str_intern("pre").to_value())
                );
                assert!(
                    after
                        .to_value()
                        .ptr_eq(ctx.frozen_heap().alloc_str_intern("post").to_value())
                );
            }
            _ => panic!("wrong terminal operation"),
        }
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
            "TERMINAL_ARG route={route} symbols={} payload_bytes={} shared={shared}",
            actual.len(),
            text.iter().map(String::len).sum::<usize>()
        );
        shared_counts.push(shared);
    }
    assert_eq!(shared_counts, [3, 3]);
}

fn argument_pointers(args: &ArgsCompiledValue) -> Vec<*const u8> {
    args.arg_exprs()
        .flat_map(|e| symbols(e).into_iter().map(|entry| entry.1))
        .chain(args.names.iter().map(|entry| entry.0.as_str().as_ptr()))
        .collect()
}

#[test]
fn rejected_terminal_checks_preserve_arguments_and_full_call_fallback() {
    let module = Module::new();
    let spans = spans(&module);
    let mut eval = Evaluator::new(&module);
    let mut ctx = OptCtx::new(&mut eval, 8);
    for route in ["type_is", "format"] {
        let fun = IrSpanned {
            span: spans[4],
            node: if route == "type_is" {
                frozen_function(&module, "is_list")
            } else {
                bound_method(&module, "pre{}post", "format", &mut ctx)
            },
        };
        for shape in 0..6 {
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
                _ => unreachable!(),
            }
            let expected = format!("{args:?}");
            let pointers = argument_pointers(&args);
            let output = if route == "type_is" {
                CallCompiled::try_type_is(&fun, &mut args)
            } else {
                CallCompiled::try_format(&fun, &mut args, &mut ctx)
            };
            assert!(output.is_none());
            assert_eq!(format!("{args:?}"), expected);
            assert_eq!(argument_pointers(&args), pointers);
        }
    }
    for (route, node) in [
        ("type_is", local(spans[4]).node),
        ("type_is", frozen_function(&module, "ordinary")),
        ("format", local(spans[4]).node),
        (
            "format",
            bound_method(&module, "pre{}post", "lower", &mut ctx),
        ),
        ("format", bound_method(&module, "{}{}", "format", &mut ctx)),
    ] {
        let fun = IrSpanned {
            span: spans[4],
            node,
        };
        let mut args = ArgsCompiledValue {
            pos_named: vec![dot(spans[1], local(spans[0]), "field")],
            ..Default::default()
        };
        let expected = format!("{args:?}");
        let pointers = argument_pointers(&args);
        let output = if route == "type_is" {
            CallCompiled::try_type_is(&fun, &mut args)
        } else {
            CallCompiled::try_format(&fun, &mut args, &mut ctx)
        };
        assert!(output.is_none());
        assert_eq!(format!("{args:?}"), expected);
        assert_eq!(argument_pointers(&args), pointers);
        if route == "format"
            && fun
                .as_frozen_bound_method()
                .is_some_and(|m| m.method.name == "format")
        {
            let expected_fun = format!("{fun:?}");
            let output = CallCompiled::call(spans[6], spans[7], fun, args, &mut ctx);
            let ExprCompiled::Call(call) = output else {
                panic!("invalid format must retain ordinary call");
            };
            assert_eq!(call.span, spans[6]);
            assert_eq!(call.node.call_site, spans[7]);
            assert_eq!(format!("{:?}", call.node.fun), expected_fun);
            assert_eq!(format!("{:?}", call.node.args), expected);
            assert_eq!(argument_pointers(&call.node.args), pointers);
        }
    }
}

#[test]
fn terminal_calls_preserve_effects_errors_and_retained_frozen_functions() {
    let source = String::from(
        r#"
def is_list(x):
    return type(x) == type([])
def seen(log, value):
    log.append("value")
    return value
def check(log, value):
    return is_list(seen(log, value).outer.inner)
def named_check(log, value):
    return is_list(x=seen(log, value).outer.inner)
def star_check(log, values):
    return is_list(*seen(log, values))
def render(log, value):
    return "pre{}post".format(seen(log, value).outer.inner)
def broken(log, value):
    return "{}{}".format(seen(log, value))
log = []
assert_eq(check(log, struct(outer=struct(inner=[]))), True)
assert_eq(render(log, struct(outer=struct(inner="é"))), "preépost")
assert_eq(log, ["value", "value"])
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions = [
        "is_list",
        "check",
        "named_check",
        "star_check",
        "render",
        "broken",
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
    assertion.pass(
        r#"
log = []
RESULT = []
for value, expected in [([], True), ([1], True), ({}, False), ("é", False), (1, False)]:
    receiver = struct(outer=struct(inner=value))
    RESULT.append(check(log, receiver))
    assert_eq(named_check(log, receiver), expected)
    assert_eq(star_check(log, [value]), expected)
RESULT.append(render(log, struct(outer=struct(inner="é世界"))))
RESULT.append(render(log, struct(outer=struct(inner=123))))
receiver = None
value = None
garbage_collect()
assert_eq(RESULT, [True, True, False, False, False, "preé世界post", "pre123post"])
assert_eq(log, ["value"] * 17)
"#,
    );
    assertion.disable_static_typechecking();
    let errors: Vec<_> = [
        "check([], struct())",
        "render([], struct(outer=struct()))",
        "star_check([], [])",
        "star_check([], [[], {}])",
        "broken([], 1)",
    ]
    .into_iter()
    .map(|program| format!("{:#}", assertion.fails(program, &[])))
    .collect();
    println!(
        "TERMINAL_ARG_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}
