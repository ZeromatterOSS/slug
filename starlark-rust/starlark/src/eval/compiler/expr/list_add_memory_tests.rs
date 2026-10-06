use super::*;
use crate::assert::Assert;
use crate::codemap::CodeMap;
use crate::codemap::Pos;
use crate::codemap::Span;
use crate::environment::Module;
use crate::values::types::list::AllocList;

fn spans(module: &Module) -> [FrameSpan; 6] {
    let file = module.frozen_heap().alloc_any(CodeMap::new(
        "list-add-memory.star".into(),
        "0123456789abcdef".into(),
    ));
    std::array::from_fn(|i| {
        FrameSpan::new(FrozenFileSpan::new(
            file,
            Span::new(Pos::new(i as u32), Pos::new(i as u32 + 1)),
        ))
    })
}

fn local(span: FrameSpan) -> IrSpanned<ExprCompiled> {
    IrSpanned {
        span,
        node: ExprCompiled::Local(LocalSlotId(7)),
    }
}

fn dynamic(spans: &[FrameSpan; 6], tag: &str) -> IrSpanned<ExprCompiled> {
    let mut expression = local(spans[0]);
    for i in 0..3 {
        expression = IrSpanned {
            span: spans[i + 1],
            node: ExprCompiled::Builtin1(
                Builtin1::Dot(Symbol::new(&format!("{tag}_{i}_{}", "owner".repeat(1024)))),
                Box::new(expression),
            ),
        };
    }
    expression
}

fn pointers(expression: &IrSpanned<ExprCompiled>) -> Vec<(*const u8, usize)> {
    match &expression.node {
        ExprCompiled::List(xs) | ExprCompiled::Tuple(xs) => xs.iter().flat_map(pointers).collect(),
        ExprCompiled::Builtin1(Builtin1::Dot(symbol), inner) => {
            let mut result = vec![(symbol.as_str().as_ptr(), symbol.as_str().len())];
            result.extend(pointers(inner));
            result
        }
        _ => Vec::new(),
    }
}

fn list(span: FrameSpan, xs: Vec<IrSpanned<ExprCompiled>>) -> IrSpanned<ExprCompiled> {
    IrSpanned {
        span,
        node: ExprCompiled::List(xs),
    }
}

fn frozen_list(module: &Module, span: FrameSpan, count: usize) -> IrSpanned<ExprCompiled> {
    let value = module.frozen_heap().alloc(AllocList(std::iter::repeat_n(
        module.frozen_heap().alloc(17),
        count,
    )));
    IrSpanned {
        span,
        node: ExprCompiled::Value(value),
    }
}

#[test]
fn list_add_moves_symbols_and_preserves_elements_spans_and_frozen_constants() {
    let module = Module::new();
    let spans = spans(&module);
    let mut eval = Evaluator::new(&module);
    let mut counts = Vec::new();
    for route in 0..4 {
        let left = if route == 2 {
            frozen_list(&module, spans[4], 2)
        } else {
            list(spans[4], vec![dynamic(&spans, "left")])
        };
        let right = match route {
            0 => list(spans[5], vec![dynamic(&spans, "right")]),
            1 => frozen_list(&module, spans[5], 2),
            2 => list(spans[5], vec![dynamic(&spans, "right")]),
            _ => list(spans[5], Vec::new()),
        };
        let originals: Vec<_> = pointers(&left)
            .into_iter()
            .chain(pointers(&right))
            .collect();
        let expected: Vec<_> = left
            .as_short_list()
            .unwrap()
            .as_exprs()
            .into_iter()
            .chain(right.as_short_list().unwrap().as_exprs())
            .collect();
        let output =
            ExprCompiled::bin_op(Builtin2::Add, left, right, &mut OptCtx::new(&mut eval, 8));
        let ExprCompiled::List(actual) = output else {
            panic!("expected short-list concatenation");
        };
        assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        for (actual, expected) in actual.iter().zip(&expected) {
            assert_eq!(actual.span, expected.span);
            if let ExprCompiled::Value(value) = actual.node {
                assert!(
                    value
                        .to_value()
                        .ptr_eq(expected.as_value().unwrap().to_value())
                );
            }
        }
        let actual: Vec<_> = actual.iter().flat_map(pointers).collect();
        let shared = actual
            .iter()
            .zip(&originals)
            .filter(|(a, b)| a.0 == b.0)
            .count();
        println!(
            "LIST_ADD route={route} symbols={} payload_bytes={} shared={shared}",
            originals.len(),
            originals.iter().map(|x| x.1).sum::<usize>()
        );
        counts.push((shared, originals.len()));
    }
    assert!(counts.iter().all(|(shared, total)| shared == total));
}

#[test]
fn list_add_preserves_size_guards_and_rejected_operand_ownership() {
    let module = Module::new();
    let spans = spans(&module);
    let owned = |count| {
        let mut xs = vec![local(spans[0]); count];
        if count != 0 {
            xs[0] = dynamic(&spans, "guard");
        }
        list(spans[4], xs)
    };
    for left_frozen in [false, true] {
        for right_frozen in [false, true] {
            let left = if left_frozen {
                frozen_list(&module, spans[4], 1000)
            } else {
                owned(1000)
            };
            let right = if right_frozen {
                frozen_list(&module, spans[5], 1000)
            } else {
                owned(1000)
            };
            let output = ExprCompiled::add(left, right);
            let ExprCompiled::List(xs) = output else {
                panic!("1000 per side must fold");
            };
            assert_eq!(xs.len(), 2000);
        }
    }
    for shape in 0..8 {
        let (left, right) = match shape {
            0 => (owned(1001), owned(0)),
            1 => (owned(0), owned(1001)),
            2 => (frozen_list(&module, spans[4], 1001), owned(1)),
            3 => (owned(1), frozen_list(&module, spans[5], 1001)),
            4 => (local(spans[4]), owned(1)),
            5 => (owned(1), local(spans[5])),
            6 => (
                IrSpanned {
                    span: spans[4],
                    node: ExprCompiled::Tuple(vec![dynamic(&spans, "tuple")]),
                },
                owned(1),
            ),
            _ => (
                owned(1),
                IrSpanned {
                    span: spans[5],
                    node: ExprCompiled::Value(
                        module.frozen_heap().alloc_str("é").to_frozen_value(),
                    ),
                },
            ),
        };
        let expected = (format!("{left:?}"), format!("{right:?}"));
        let original = (pointers(&left), pointers(&right));
        let ExprCompiled::Builtin2(Builtin2::Add, operands) = ExprCompiled::add(left, right) else {
            panic!("shape {shape} must preserve ordinary Add");
        };
        assert_eq!(
            (format!("{:?}", operands.0), format!("{:?}", operands.1)),
            expected
        );
        assert_eq!((pointers(&operands.0), pointers(&operands.1)), original);
    }
    let ExprCompiled::List(xs) = ExprCompiled::add(list(spans[4], vec![]), list(spans[5], vec![]))
    else {
        panic!("empty lists must fold");
    };
    assert!(xs.is_empty());
}

#[test]
fn list_add_preserves_effects_errors_and_retained_frozen_functions() {
    let source = String::from(
        r#"
def seen(log, tag, value):
    log.append(tag)
    return value
def joined(log, value):
    return [seen(log, "left", value).first] + [seen(log, "right", value).second]
def mixed(log, value):
    return ["é", "世界"] + [seen(log, "mixed", value)] + []
def ordinary(left, right):
    return left + right
log = []
assert_eq(joined(log, struct(first=1, second=2)), [1, 2])
assert_eq(log, ["left", "right"])
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions = ["joined", "mixed", "ordinary"].map(|name| (name, module.get(name).unwrap()));
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
RESULT = joined(log, struct(first="é", second="世界"))
assert_eq(RESULT, ["é", "世界"])
assert_eq(log, ["left", "right"])
garbage_collect()
RESULT.append("retained")
assert_eq(RESULT, ["é", "世界", "retained"])
log = []
assert_eq(mixed(log, 3), ["é", "世界", 3])
assert_eq(log, ["mixed"])
left, right = [1], [2]
result = ordinary(left, right)
result.append(3)
assert_eq(left, [1])
assert_eq(right, [2])
assert_eq(result, [1, 2, 3])
garbage_collect()
"#,
    );
    assertion.disable_static_typechecking();
    let errors: Vec<_> = [
        "joined([], struct())",
        "joined([], struct(first=1))",
        "ordinary([], 1)",
        "ordinary(1, [])",
    ]
    .into_iter()
    .map(|program| format!("{:#}", assertion.fails(program, &[])))
    .collect();
    println!(
        "LIST_ADD_ERRORS={}",
        serde_json::to_string(&errors).unwrap()
    );
}
