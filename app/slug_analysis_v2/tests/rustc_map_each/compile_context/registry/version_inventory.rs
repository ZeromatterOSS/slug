//! Potential URLs for a finite caller-authored MODULE fixture graph only.
//! The real Bzlmod evaluator still owns all discovery, alias, and override
//! semantics. This uses its existing syntax parser, never a replacement grammar.
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::ast::AstExpr;
use starlark::syntax::ast::AstLiteral;
use starlark::syntax::ast::Expr;
use starlark::syntax::ast::Stmt;

fn string(expr: &AstExpr) -> &str {
    let Expr::Literal(AstLiteral::String(value)) = &expr.node else {
        panic!("fixture registry version inventory requires literal strings")
    };
    &value.node
}

pub(super) fn requests(source: &str) -> Vec<(String, String)> {
    let ast = AstModule::parse(
        "fixture/MODULE.bazel",
        source.to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let statements = match &ast.statement().node {
        Stmt::Statements(statements) => statements.as_slice(),
        _ => std::slice::from_ref(ast.statement()),
    };
    let mut requests = Vec::new();
    for statement in statements {
        let expression = match &statement.node {
            Stmt::Expression(expression) => expression,
            Stmt::Assign(assignment) => &assignment.rhs,
            _ => panic!("fixture registry inventory requires top-level directives"),
        };
        let Expr::Call(function, arguments) = &expression.node else {
            continue;
        };
        let Expr::Identifier(function) = &function.node else {
            continue;
        };
        let operation = function.node.ident.as_str();
        assert_ne!(
            operation, "include",
            "prepare included fixture MODULE fragments explicitly"
        );
        if !matches!(
            operation,
            "bazel_dep" | "single_version_override" | "multiple_version_override"
        ) {
            continue;
        }
        // Irrelevant kwargs are not interpreted, including repo_name=None.
        let argument = |name: &str| {
            arguments
                .args
                .iter()
                .find_map(|argument| {
                    (argument.node.name() == Some(name)).then(|| argument.node.expr())
                })
                .unwrap_or_else(|| panic!("fixture registry inventory lacks {name}"))
        };
        let name = string(argument(if operation == "bazel_dep" {
            "name"
        } else {
            "module_name"
        }));
        if operation == "multiple_version_override" {
            let Expr::List(versions) = &argument("versions").node else {
                panic!("fixture registry inventory requires a literal versions list")
            };
            requests.extend(
                versions
                    .iter()
                    .map(|version| (name.to_owned(), string(version).to_owned())),
            );
        } else {
            requests.push((name.to_owned(), string(argument("version")).to_owned()));
        }
    }
    requests
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_ignores_unneeded_kwargs_and_refuses_dynamic_required_inputs() {
        assert_eq!(
            requests(
                "module(name='root')\nbazel_dep(name='dep', version='1', repo_name=None)\nsingle_version_override(module_name='dep', version='2')\nmultiple_version_override(module_name='dep', versions=['3','4'])\n"
            ),
            vec![
                ("dep".to_owned(), "1".to_owned()),
                ("dep".to_owned(), "2".to_owned()),
                ("dep".to_owned(), "3".to_owned()),
                ("dep".to_owned(), "4".to_owned())
            ]
        );
        for source in [
            "bazel_dep(name='dep', version=chosen)\n",
            "multiple_version_override(module_name='dep', versions=chosen)\n",
            "include('//:extra.MODULE.bazel')\n",
        ] {
            assert!(std::panic::catch_unwind(|| requests(source)).is_err());
        }
    }
}
