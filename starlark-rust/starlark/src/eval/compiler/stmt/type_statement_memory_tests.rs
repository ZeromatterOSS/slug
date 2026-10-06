use super::*;
use crate::assert::Assert;
use crate::codemap::CodeMap;
use crate::codemap::Pos;
use crate::collections::symbol::symbol::Symbol;
use crate::environment::Globals;
use crate::environment::Module;
use crate::eval::compiler::args::ArgsCompiledValue;
use crate::eval::compiler::call::CallCompiled;
use crate::values::list::ListRef;

fn spans(module: &Module) -> [FrameSpan; 8] {
    let file = module.frozen_heap().alloc_any(CodeMap::new(
        "type-statement-memory.star".into(),
        "0123456789abcdef".into(),
    ));
    std::array::from_fn(|i| {
        FrameSpan::new(FrozenFileSpan::new(
            file,
            Span::new(Pos::new(i as u32), Pos::new(i as u32 + 1)),
        ))
    })
}

fn argument(spans: &[FrameSpan; 8]) -> IrSpanned<ExprCompiled> {
    let mut e = IrSpanned {
        span: spans[0],
        node: ExprCompiled::Local(LocalSlotId(7)),
    };
    for i in 0..3 {
        e = IrSpanned {
            span: spans[i + 1],
            node: ExprCompiled::Builtin1(
                Builtin1::Dot(Symbol::new(&format!("field_{i}_{}", "owner".repeat(1024)))),
                Box::new(e),
            ),
        };
    }
    e
}

fn symbols(mut e: &IrSpanned<ExprCompiled>) -> Vec<(&str, *const u8)> {
    let mut result = Vec::new();
    while let ExprCompiled::Builtin1(Builtin1::Dot(name), object) = &e.node {
        result.push((name.as_str(), name.as_str().as_ptr()));
        e = object;
    }
    assert!(matches!(e.node, ExprCompiled::Local(LocalSlotId(7))));
    result
}

#[test]
fn type_statements_transfer_symbols_after_compile_and_owned_freeze_optimization() {
    let module = Module::new();
    let globals = Globals::standard();
    let spans = spans(&module);
    let mut eval = Evaluator::new(&module);
    let mut shared = Vec::new();
    for optimized in [false, true] {
        let mut ctx = OptCtx::new(&mut eval, 8);
        let mut expression = IrSpanned {
            span: spans[6],
            node: CallCompiled::call(
                spans[6],
                spans[5],
                IrSpanned {
                    span: spans[4],
                    node: ExprCompiled::Value(globals.get_frozen("type").unwrap()),
                },
                ArgsCompiledValue {
                    pos_named: vec![argument(&spans)],
                    ..Default::default()
                },
                &mut ctx,
            ),
        };
        if optimized {
            expression = expression.optimize(&mut ctx);
        }
        // Snapshot only the freshly owned lowering input, after legitimate borrowed optimization.
        let arg = expression.as_type().unwrap();
        let expected = format!("{arg:?}");
        let span = arg.span;
        let pointers = symbols(arg).iter().map(|x| x.1).collect::<Vec<_>>();
        let text = symbols(arg)
            .iter()
            .map(|x| x.0.to_owned())
            .collect::<Vec<_>>();
        let result = StmtsCompiled::expr(expression);
        assert_eq!(result.stmts().len(), 1);
        let stmt = result.first().unwrap();
        assert_eq!(stmt.span, span);
        let StmtCompiled::Expr(actual) = &stmt.node else {
            panic!("expected argument statement")
        };
        assert_eq!(format!("{actual:?}"), expected);
        let actual = symbols(actual);
        assert_eq!(
            actual.iter().map(|x| x.0).collect::<Vec<_>>(),
            text.iter().map(String::as_str).collect::<Vec<_>>()
        );
        let count = actual
            .iter()
            .zip(pointers)
            .filter(|(a, b)| a.1 == *b)
            .count();
        println!(
            "TYPE_STATEMENT optimized={optimized} symbols={} content_bytes={} shared={count}",
            actual.len(),
            text.iter().map(String::len).sum::<usize>()
        );
        shared.push(count);
    }
    assert_eq!(shared, [3, 3]);
}

fn pointers(e: &IrSpanned<ExprCompiled>) -> Vec<*const u8> {
    match &e.node {
        ExprCompiled::Call(c) => c
            .node
            .args
            .arg_exprs()
            .flat_map(|e| symbols(e).into_iter().map(|x| x.1))
            .chain(c.node.args.names.iter().map(|x| x.0.as_str().as_ptr()))
            .collect(),
        ExprCompiled::Builtin1(Builtin1::Dot(_), _) => {
            symbols(e).into_iter().map(|x| x.1).collect()
        }
        _ => Vec::new(),
    }
}

#[test]
fn rejected_type_statement_shapes_keep_complete_ir_owners_and_pure_elision() {
    let module = Module::new();
    let globals = Globals::standard();
    let spans = spans(&module);
    for shape in 0..9 {
        let mut args = ArgsCompiledValue::default();
        match shape {
            0 => {}
            1 => args.pos_named = vec![argument(&spans), argument(&spans)],
            2 => {
                args.pos_named = vec![argument(&spans)];
                args.names = vec![(Symbol::new("x"), module.frozen_heap().alloc_str("x"))];
            }
            3 => {
                args.pos_named = vec![argument(&spans)];
                args.args = Some(argument(&spans));
            }
            4 => {
                args.pos_named = vec![argument(&spans)];
                args.kwargs = Some(argument(&spans));
            }
            5 => {
                args.args = Some(argument(&spans));
                args.kwargs = Some(argument(&spans));
            }
            _ => args.pos_named = vec![argument(&spans)],
        }
        let fun = if shape == 7 {
            ExprCompiled::Local(LocalSlotId(2))
        } else {
            ExprCompiled::Value(
                globals
                    .get_frozen(if shape == 6 { "len" } else { "type" })
                    .unwrap(),
            )
        };
        let expression = if shape == 8 {
            argument(&spans)
        } else {
            IrSpanned {
                span: spans[6],
                node: ExprCompiled::Call(Box::new(IrSpanned {
                    span: spans[5],
                    node: CallCompiled {
                        call_site: spans[4],
                        fun: IrSpanned {
                            span: spans[4],
                            node: fun,
                        },
                        args,
                    },
                })),
            }
        };
        assert!(expression.as_type().is_none());
        let expected = format!("{expression:?}");
        let expected_pointers = pointers(&expression);
        let span = expression.span;
        let result = StmtsCompiled::expr(expression);
        assert_eq!(result.stmts().len(), 1);
        assert_eq!(result.first().unwrap().span, span);
        let StmtCompiled::Expr(actual) = &result.first().unwrap().node else {
            panic!("expected fallback")
        };
        assert_eq!(format!("{actual:?}"), expected);
        assert_eq!(pointers(actual), expected_pointers, "shape={shape}");
    }
    let pure = IrSpanned {
        span: spans[6],
        node: ExprCompiled::Call(Box::new(IrSpanned {
            span: spans[5],
            node: CallCompiled {
                call_site: spans[4],
                fun: IrSpanned {
                    span: spans[4],
                    node: ExprCompiled::Value(globals.get_frozen("type").unwrap()),
                },
                args: ArgsCompiledValue {
                    pos_named: vec![IrSpanned {
                        span: spans[0],
                        node: ExprCompiled::Value(FrozenValue::new_bool(true)),
                    }],
                    ..Default::default()
                },
            },
        })),
    };
    assert!(pure.as_type().is_some());
    assert!(pure.is_pure_infallible());
    assert!(StmtsCompiled::expr(pure).is_empty());
    println!("TYPE_STATEMENT_REJECTIONS=9");
}

#[test]
fn type_statements_preserve_effects_errors_shadowing_and_retained_frozen_functions() {
    let source = String::from(
        r#"
def seen(log, tag, value):
    log.append(tag)
    return value
def direct(log, value):
    type(seen(log, "direct", value).outer.inner)
def nested(log, value):
    not type(seen(log, "not", value))
    [type(seen(log, "list", value)), type(seen(log, "list2", value))]
    (type(seen(log, "tuple", value)),)
def logical(log, value):
    False and type(seen(log, "skip", value))
    True and type(seen(log, "and", value))
    True or type(seen(log, "skip2", value))
    False or type(seen(log, "or", value))
def star(log, values): type(*seen(log, "star", values))
def named(log, values): type(**seen(log, "named", values))
def zero(log): type()
def two(log, value): type(seen(log, "left", value), seen(log, "right", value))
def callback(log): log.append("callee")
def shadow(type, log): type(seen(log, "shadow", log))
log = []
direct(log, struct(outer=struct(inner=[])))
nested(log, "é世界")
logical(log, 7)
shadow(callback, log)
assert_eq(log, ["direct", "not", "list", "list2", "tuple", "and", "or", "shadow", "callee"])
garbage_collect()
"#,
    );
    let mut primary = Assert::new();
    primary.disable_static_typechecking();
    let module = primary.pass_module(&source);
    let two = module.get("two").unwrap();
    let functions = [
        "direct", "nested", "logical", "star", "named", "zero", "callback", "shadow",
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
for value in [[], {}, "é世界", 7]:
    direct(log, struct(outer=struct(inner=value)))
    nested(log, value)
    logical(log, value)
    star(log, [value])
shadow(callback, log)
value = None
garbage_collect()
assert_eq(log, ["direct", "not", "list", "list2", "tuple", "and", "or", "star"] * 4 + ["shadow", "callee"])
"#);
    assertion.disable_static_typechecking();
    let mut errors = [
        "direct([], struct())",
        "direct([], struct(outer=struct()))",
        "star([], [])",
        "star([], [[], {}])",
        "named([], {'x': []})",
        "zero([])",
    ]
    .into_iter()
    .map(|program| format!("{:#}", assertion.fails(program, &[])))
    .collect::<Vec<_>>();
    let runtime = Module::new();
    let log = runtime.heap().alloc(Vec::<String>::new());
    let mut eval = Evaluator::new(&runtime);
    errors.push(format!(
        "{:#}",
        eval.eval_function(two.value(), &[log, runtime.heap().alloc(7)], &[])
            .unwrap_err()
    ));
    assert_eq!(
        ListRef::from_value(log)
            .unwrap()
            .iter()
            .map(|v| v.unpack_str().unwrap())
            .collect::<Vec<_>>(),
        ["left", "right"]
    );
    assert_eq!(errors.len(), 7);
    println!(
        "TYPE_STATEMENT_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}
