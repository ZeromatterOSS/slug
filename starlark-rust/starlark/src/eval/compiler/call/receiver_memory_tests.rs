use super::*;
use crate::assert::Assert;
use crate::codemap::CodeMap;
use crate::codemap::Pos;
use crate::codemap::Span;
use crate::environment::Module;
use crate::eval::Evaluator;
use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
use crate::eval::runtime::slots::LocalSlotId;

fn nested_symbols(mut expression: &IrSpanned<ExprCompiled>) -> Vec<(&str, *const u8)> {
    let mut symbols = Vec::new();
    while let ExprCompiled::Builtin1(Builtin1::Dot(field), object) = &expression.node {
        symbols.push((field.as_str(), field.as_str().as_ptr()));
        expression = object;
    }
    assert!(matches!(
        expression.node,
        ExprCompiled::Local(LocalSlotId(7))
    ));
    symbols
}

#[test]
fn method_call_transfers_nested_symbol_buffers_and_preserves_ir_spans_arguments() {
    let module = Module::new();
    let file = module.frozen_heap().alloc_any(CodeMap::new(
        "receiver-memory.star".into(),
        "0123456789abcdef".into(),
    ));
    let spans: [FrameSpan; 8] = std::array::from_fn(|index| {
        FrameSpan::new(FrozenFileSpan::new(
            file,
            Span::new(Pos::new(index as u32), Pos::new(index as u32 + 1)),
        ))
    });
    let mut receiver = IrSpanned {
        span: spans[0],
        node: ExprCompiled::Local(LocalSlotId(7)),
    };
    for index in 0..3 {
        let name = format!("field_{index}_{}", "owner".repeat(1024));
        receiver = IrSpanned {
            span: spans[index + 1],
            node: ExprCompiled::Builtin1(Builtin1::Dot(Symbol::new(&name)), Box::new(receiver)),
        };
    }
    let pointers: Vec<_> = nested_symbols(&receiver)
        .iter()
        .map(|entry| entry.1)
        .collect();
    let expected = format!("{receiver:?}");
    let names: Vec<_> = nested_symbols(&receiver)
        .iter()
        .map(|entry| entry.0.to_owned())
        .collect();
    let field = Symbol::new("run");
    let outer_pointer = field.as_str().as_ptr();
    let fun = IrSpanned {
        span: spans[4],
        node: ExprCompiled::Builtin1(Builtin1::Dot(field), Box::new(receiver)),
    };
    let args = ArgsCompiledValue {
        pos_named: vec![IrSpanned {
            span: spans[7],
            node: ExprCompiled::Local(LocalSlotId(1)),
        }],
        names: vec![(
            Symbol::new("named"),
            module.frozen_heap().alloc_str("named"),
        )],
        args: Some(IrSpanned {
            span: spans[6],
            node: ExprCompiled::Local(LocalSlotId(2)),
        }),
        kwargs: Some(IrSpanned {
            span: spans[5],
            node: ExprCompiled::Local(LocalSlotId(3)),
        }),
    };
    let expected_args = format!("{args:?}");
    let mut eval = Evaluator::new(&module);
    let output = CallCompiled::call(
        spans[5],
        spans[6],
        fun,
        args,
        &mut OptCtx::new(&mut eval, 8),
    );
    let ExprCompiled::Call(call) = output else {
        panic!("expected dynamic method call")
    };
    assert_eq!(call.span, spans[5]);
    assert_eq!(call.node.call_site, spans[6]);
    assert_eq!(call.node.fun.span, spans[4]);
    let (receiver, method, args) = call.node.method().unwrap();
    assert_eq!(format!("{receiver:?}"), expected);
    assert_eq!(format!("{args:?}"), expected_args);
    assert_eq!(method.as_str(), "run");
    assert_ne!(method.as_str().as_ptr(), outer_pointer);
    let actual = nested_symbols(receiver);
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
        "METHOD_RECEIVER symbols={} payload_bytes={} shared={shared}",
        actual.len(),
        names.iter().map(String::len).sum::<usize>()
    );
    assert_eq!(shared, 3);
}

#[test]
fn ordinary_call_keeps_function_arguments_and_span_ownership() {
    let module = Module::new();
    let fun = IrSpanned {
        span: Default::default(),
        node: ExprCompiled::Local(LocalSlotId(2)),
    };
    let args = ArgsCompiledValue {
        pos_named: vec![IrSpanned {
            span: Default::default(),
            node: ExprCompiled::Local(LocalSlotId(1)),
        }],
        names: vec![(
            Symbol::new("argument"),
            module.frozen_heap().alloc_str("argument"),
        )],
        ..Default::default()
    };
    let pointer = args.names[0].0.as_str().as_ptr();
    let expected_fun = format!("{fun:?}");
    let expected_args = format!("{args:?}");
    let mut eval = Evaluator::new(&module);
    let output = CallCompiled::call(
        Default::default(),
        Default::default(),
        fun,
        args,
        &mut OptCtx::new(&mut eval, 3),
    );
    let ExprCompiled::Call(call) = output else {
        panic!("expected ordinary call")
    };
    assert_eq!(format!("{:?}", call.node.fun), expected_fun);
    assert_eq!(format!("{:?}", call.node.args), expected_args);
    assert_eq!(call.node.args.names[0].0.as_str().as_ptr(), pointer);
    assert!(call.node.method().is_none());
}

#[test]
fn nested_method_execution_preserves_order_errors_and_frozen_ownership() {
    let source = String::from(
        r#"
def method(first, second=0):
    return first + second
def invoke(receiver, first, second):
    return receiver.outer.inner.method(first, second=second)
def seen(log, tag, value):
    log.append(tag)
    return value
def ordered(log, receiver):
    return seen(log, "receiver", receiver).outer.inner.method(seen(log, "pos", 3), second=seen(log, "kw", 4))
receiver = struct(outer=struct(inner=struct(method=method)))
log = []
assert_eq(invoke(receiver, 2, 5), 7)
assert_eq(ordered(log, receiver), 7)
assert_eq(log, ["receiver", "pos", "kw"])
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions = ["method", "invoke", "ordered"].map(|name| (name, module.get(name).unwrap()));
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let mut assert = Assert::new();
    assert.globals_add(|builder| {
        for (name, function) in functions {
            builder.set(name, function);
        }
    });
    assert.pass(
        r#"
receiver = struct(outer=struct(inner=struct(method=method)))
log = []
RESULT = [invoke(receiver, 2, 5), ordered(log, receiver), invoke(receiver, "é", "世界")]
garbage_collect()
assert_eq(RESULT, [7, 7, "é世界"])
assert_eq(log, ["receiver", "pos", "kw"])
"#,
    );
    assert.fails(
        "invoke(struct(outer=struct(inner=struct())), 1, 2)",
        &["Object of type `struct` has no attribute `method`"],
    );
    assert.fails(
        "invoke(struct(outer=struct(inner=struct(method=method))), 1, 'bad')",
        &["Operation `+` not supported for types `int` and `string`"],
    );
}
