/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_query_v2::QueryArg;
use slug_query_v2::QueryExpression;
use slug_query_v2::QueryFunctionName;
use slug_query_v2::QueryParseError;

#[test]
fn parses_nested_query_functions() {
    let expression = QueryExpression::parse("kind(\"cc_.* rule\", deps(//pkg:bin))").unwrap();

    let QueryExpression::Function(function) = expression else {
        panic!("expected function expression");
    };
    assert_eq!(function.name, QueryFunctionName::Kind);
    assert_eq!(
        function.args[0],
        QueryArg::StringLiteral("cc_.* rule".to_owned())
    );
    let QueryArg::Expr(nested) = &function.args[1] else {
        panic!("expected nested expression");
    };
    assert_eq!(nested.to_string(), "deps(//pkg:bin)");
}

#[test]
fn parses_query_words_without_treating_them_as_targets() {
    let expression = QueryExpression::parse("rdeps(//..., //pkg:lib, 1)").unwrap();

    let QueryExpression::Function(function) = expression else {
        panic!("expected function expression");
    };
    assert_eq!(function.name, QueryFunctionName::Rdeps);
    assert_eq!(function.args[2], QueryArg::Word("1".to_owned()));
}

#[test]
fn rejects_unsupported_functions() {
    let err = QueryExpression::parse("somepath(//a:b, //c:d)").unwrap_err();

    assert!(matches!(
        err,
        QueryParseError::UnsupportedFunction { name, .. } if name == "somepath"
    ));
}
