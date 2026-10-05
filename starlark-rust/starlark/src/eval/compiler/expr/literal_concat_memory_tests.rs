use starlark_syntax::syntax::ast::AstExpr;
use starlark_syntax::syntax::ast::AstNoPayload;

use super::*;
use crate::assert::Assert;
use crate::environment::Globals;
use crate::environment::Module;
use crate::syntax::AstModule;
use crate::syntax::Dialect;
use crate::syntax::StringEncoding;

fn literal(text: &str) -> AstExpr {
    Spanned {
        node: ExprP::Literal(AstLiteral::String(Spanned {
            node: text.to_owned(),
            span: Default::default(),
        })),
        span: Default::default(),
    }
}

fn operation(left: AstExpr, op: BinOp, right: AstExpr) -> AstExpr {
    Spanned {
        node: ExprP::Op(Box::new(left), op, Box::new(right)),
        span: Default::default(),
    }
}

fn reduce(expr: &AstExpr) -> Option<String> {
    let ExprP::Op(left, op, right) = &expr.node else {
        panic!("expected binary operation");
    };
    ExprP::<AstNoPayload>::reduces_to_string(*op, left, right)
}

#[test]
fn literal_chain_order_rejection_and_owned_result() {
    for parts in [vec!["", ""], vec!["é", "", "\0", "世界", "tail"]] {
        let mut tree = literal(parts[0]);
        for part in &parts[1..] {
            tree = operation(tree, BinOp::Add, literal(part));
        }
        let result = reduce(&tree).unwrap();
        drop(tree);
        assert_eq!(result, parts.concat());
    }
    let mut tree = literal("start");
    let mut expected = String::from("start");
    for i in 0..128 {
        let part = format!("é{i}\0");
        tree = operation(tree, BinOp::Add, literal(&part));
        expected.push_str(&part);
    }
    let result = reduce(&tree).unwrap();
    drop(tree);
    assert_eq!(result, expected);

    let nonliteral = Spanned {
        node: ExprP::Tuple(Vec::new()),
        span: Default::default(),
    };
    for tree in [
        operation(literal("a"), BinOp::Multiply, literal("b")),
        operation(nonliteral.clone(), BinOp::Add, literal("b")),
        operation(literal("a"), BinOp::Add, nonliteral.clone()),
        operation(
            operation(literal("a"), BinOp::Multiply, literal("b")),
            BinOp::Add,
            literal("visited"),
        ),
        operation(
            operation(nonliteral, BinOp::Add, literal("b")),
            BinOp::Add,
            literal("visited"),
        ),
        operation(
            literal("a"),
            BinOp::Add,
            operation(literal("b"), BinOp::Add, literal("c")),
        ),
    ] {
        assert_eq!(reduce(&tree), None);
    }
}

#[test]
fn compiled_literal_chain_retains_frozen_ownership_and_fallback() {
    let source = String::from(
        r#"
def folded():
    return "é" + "" + "\0" + "世界"
def mixed(x):
    return "é" + x + "世界"
def right_nested():
    return "é" + ("X" + "世界")
assert_eq(folded(), "é\0世界")
garbage_collect()
"#,
    );
    let module = Assert::new().pass_module(&source);
    let functions =
        ["folded", "mixed", "right_nested"].map(|name| (name, module.get(name).unwrap()));
    let clone = module.clone();
    drop(source);
    drop(module);
    drop(clone);
    let mut a = Assert::new();
    a.globals_add(|builder| {
        for (name, function) in functions {
            builder.set(name, function);
        }
    });
    a.pass(
        r#"
RESULT = [folded(), mixed("X"), right_nested()]
garbage_collect()
assert_eq(RESULT, ["é\0世界", "éX世界", "éX世界"])
"#,
    );
    a.fails(
        "mixed(1)",
        &["Operation `+` not supported for types `string` and `int`"],
    );
}

#[test]
fn compiled_literal_chain_preserves_bazel_internal_carriers() {
    let function = {
        let ast = AstModule::parse_with_string_encoding(
            "literal-concat-memory.bzl",
            "def folded():\n    return 'é' + '' + '界'\n".to_owned(),
            &Dialect::Bazel,
            StringEncoding::BazelInternal,
        )
        .unwrap();
        let module = Module::new();
        Evaluator::new(&module)
            .eval_module(ast, &Globals::standard())
            .unwrap();
        module.freeze().unwrap().get("folded").unwrap()
    };
    let caller = Module::new();
    let mut eval = Evaluator::new(&caller);
    let value = eval.eval_function(function.value(), &[], &[]).unwrap();
    assert_eq!(
        value.unpack_str().unwrap(),
        "\u{c3}\u{a9}\u{e7}\u{95}\u{8c}"
    );
}
