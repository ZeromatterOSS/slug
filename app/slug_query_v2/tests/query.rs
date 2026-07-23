/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use slug_query_v2::BinaryOperator;
use slug_query_v2::QueryExpression;
use slug_query_v2::QueryExpressionKind;
use slug_query_v2::QueryFunctionStatus;
use slug_query_v2::SourceSpan;
use slug_query_v2::loading_query_functions;
use slug_query_v2::validate_loading_query;

// Bazel 9.2 QueryParser.java: all binary operators have equal precedence and
// are left-associative. QueryParserTest.testMultipleBinaryOperatorParsing
// covers the corresponding nested shape.
#[test]
fn parser_retains_bazel_left_associative_shape_and_source_spans() {
    let expression = QueryExpression::parse("a union b except c").unwrap();
    assert_eq!(expression.span, SourceSpan { start: 0, end: 18 });
    let QueryExpressionKind::BinaryOpSequence { left, operations } = expression.kind else {
        panic!("expected binary sequence");
    };
    assert!(matches!(left.kind, QueryExpressionKind::TargetLiteral(_)));
    assert_eq!(operations.len(), 2);
    assert_eq!(operations[0].0, BinaryOperator::Union);
    assert_eq!(operations[1].0, BinaryOperator::Except);
    assert_eq!(operations[1].1.span, SourceSpan { start: 17, end: 18 });
}

// Bazel 9.2 QueryParser.java grammar plus QueryParserTest target/set cases.
#[test]
fn parser_accepts_generic_calls_let_parentheses_and_space_separated_set() {
    let expression =
        QueryExpression::parse("let x = set(//pkg:bin //pkg:lib) in (unknown($x, 1))").unwrap();
    let QueryExpressionKind::Let { name, body, .. } = expression.kind else {
        panic!("expected let");
    };
    assert_eq!(name.value, "x");
    let QueryExpressionKind::Function { name, args } = body.kind else {
        panic!("expected generic function");
    };
    assert_eq!(name.value, "unknown");
    assert_eq!(args.len(), 2);
}

// QueryEnvironment.DEFAULT_QUERY_FUNCTIONS at Bazel 9.2 is the loading-query
// registry source of truth. Loading-file provenance activates buildfiles and
// loadfiles while the other ordinary functions remain deferred.
#[test]
fn registry_distinguishes_unknown_deferred_and_validates_implemented_signatures() {
    assert_eq!(loading_query_functions().len(), 16);
    assert_eq!(
        loading_query_functions()
            .iter()
            .filter(|function| function.status == QueryFunctionStatus::Implemented)
            .map(|function| function.name)
            .collect::<Vec<_>>(),
        [
            "allpaths",
            "buildfiles",
            "deps",
            "loadfiles",
            "rdeps",
            "same_pkg_direct_rdeps",
            "siblings",
            "some",
            "somepath",
        ]
    );

    let unknown = QueryExpression::parse("not_a_bazel_query_function(//pkg:bin)").unwrap();
    assert!(
        validate_loading_query(&unknown)
            .unwrap_err()
            .to_string()
            .contains("unknown function 'not_a_bazel_query_function'")
    );
    let deferred = QueryExpression::parse("kind(rule, //pkg:all)").unwrap();
    assert!(
        validate_loading_query(&deferred)
            .unwrap_err()
            .to_string()
            .contains("recognized by Bazel 9.2")
    );
    let wrong = QueryExpression::parse("deps(//pkg:bin, 1, 2)").unwrap();
    assert!(
        validate_loading_query(&wrong)
            .unwrap_err()
            .to_string()
            .contains("too many arguments to function 'deps'")
    );
    validate_loading_query(&QueryExpression::parse("deps(//pkg:bin, 2)").unwrap()).unwrap();
    validate_loading_query(
        &QueryExpression::parse("rdeps(//tree/..., //tree/left:leaf, 2)").unwrap(),
    )
    .unwrap();
    validate_loading_query(
        &QueryExpression::parse("same_pkg_direct_rdeps(//tree/left:leaf)").unwrap(),
    )
    .unwrap();
    validate_loading_query(&QueryExpression::parse("siblings(//pkg:bin)").unwrap()).unwrap();
    validate_loading_query(
        &QueryExpression::parse("allpaths(//:linear_start, //:linear_end)").unwrap(),
    )
    .unwrap();
    validate_loading_query(
        &QueryExpression::parse("somepath(//:linear_start, //:linear_end)").unwrap(),
    )
    .unwrap();
    validate_loading_query(&QueryExpression::parse("some(//:single, '-1')").unwrap()).unwrap();
    validate_loading_query(&QueryExpression::parse("buildfiles(//pkg:bin)").unwrap()).unwrap();
    validate_loading_query(&QueryExpression::parse("loadfiles(//pkg:bin)").unwrap()).unwrap();
    assert_eq!(
        loading_query_functions()
            .iter()
            .filter(|function| function.status == QueryFunctionStatus::Deferred)
            .count(),
        7
    );
}

#[test]
fn signed_java_integer_slots_validate_without_narrowing_expression_integers() {
    for source in [
        "some(//:single, 2147483647)",
        "some(//:single, '-2147483648')",
        "deps(//:depth_root, '-1')",
        "rdeps(//..., //:depth_child, '-2147483648')",
        "some(2147483648)",
    ] {
        validate_loading_query(&QueryExpression::parse(source).unwrap()).unwrap();
    }

    for (source, raw) in [
        ("some(//:single, 2147483648)", "2147483648"),
        ("some(//:single, '-2147483649')", "-2147483649"),
        ("some(//:single, 2_147_483_647)", "2_147_483_647"),
        ("some(//:single, nope)", "nope"),
        ("deps(//:depth_root, 2147483648)", "2147483648"),
    ] {
        let error = validate_loading_query(&QueryExpression::parse(source).unwrap())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("expected an integer literal: '{raw}'")),
            "{source}: {error}"
        );
    }

    let error = QueryExpression::parse("some(//:single, -1)").unwrap_err();
    assert_eq!(error.span, SourceSpan { start: 19, end: 19 });
    assert_eq!(error.to_string(), "syntax error at '- 1 )' at bytes 19..19");
    assert_eq!(
        QueryExpression::parse("some(//:single, '-1')")
            .unwrap()
            .to_string(),
        "some(//:single, -1)"
    );
    for (source, tokens) in [
        ("some(//:single, -1foo)", "- 1foo )"),
        ("some(//:single, -1, 2)", "- 1 ,"),
        ("some(//:single, -1 union //:other)", "- 1 union"),
        ("some(//:single, -1é)", "- 1é )"),
        ("some(//:single, -1 é)", "- 1 é"),
        ("some(//:single, -1~foo)", "- 1~foo )"),
        ("some(//:single, -1[abc])", "- 1[abc] )"),
        ("some(//:single, -1#foo)", "- 1 #foo"),
        ("some(//:single, -1%foo)", "- 1 %foo"),
    ] {
        assert!(
            QueryExpression::parse(source)
                .unwrap_err()
                .to_string()
                .contains(&format!("syntax error at '{tokens}'")),
            "{source}"
        );
    }
}

#[test]
fn top_level_somepath_recognition_is_narrow_and_parentheses_lower_away() {
    for source in [
        "somepath(//:linear_start, //:linear_end)",
        "(somepath(//:linear_start, //:linear_end))",
        "((somepath(//:linear_start, //:linear_end)))",
    ] {
        assert!(
            QueryExpression::parse(source)
                .unwrap()
                .is_top_level_somepath(),
            "{source}"
        );
    }
    for source in [
        "somepath(//:linear_start, //:linear_end) union //:disconnected",
        "somepath(//:linear_start, //:linear_end) intersect set(//:linear_start //:linear_end)",
        "somepath(//:linear_start, //:linear_end) except //:linear_mid",
        "let p = somepath(//:linear_start, //:linear_end) in $p",
        "allpaths(//:linear_start, //:linear_end)",
    ] {
        assert!(
            !QueryExpression::parse(source)
                .unwrap()
                .is_top_level_somepath(),
            "{source}"
        );
    }
}

#[test]
fn parser_reports_bazel_premature_end_diagnostic_with_a_span() {
    let error = QueryExpression::parse("deps(//pkg:bin").unwrap_err();
    assert!(error.message.contains("premature end of input"));
    assert_eq!(error.span, SourceSpan { start: 14, end: 14 });
}
