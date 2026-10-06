use super::*;

fn expression(kind: QueryExpressionKind) -> QueryExpression {
    QueryExpression {
        span: SourceSpan { start: 7, end: 19 },
        kind,
    }
}

#[test]
fn integer_slots_preserve_ranges_raw_errors_capacity_and_ast_owners() {
    let boundaries = [
        0,
        1,
        i32::MAX as u64,
        i32::MAX as u64 + 1,
        u32::MAX as u64,
        u64::MAX,
    ];
    let mut numeric_cases = 0;
    for value in boundaries
        .into_iter()
        .chain((0..1024).flat_map(|n| [n, i32::MAX as u64 - 512 + n]))
    {
        let input = expression(QueryExpressionKind::Integer(value));
        let before = input.clone();
        // Decimal i32 parsing is the existing accepted contract, independent of direct conversion.
        let decimal = value.to_string();
        let expected = decimal.parse::<i32>();
        match (input.java_integer_literal(), expected) {
            (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
            (Err(actual), Err(_)) => {
                let expected = CompactString::from(decimal);
                assert_eq!(actual, expected);
                assert_eq!(actual.capacity(), expected.capacity());
            }
            (actual, expected) => panic!("{value}: {actual:?} != {expected:?}"),
        }
        assert_eq!(input, before);
        numeric_cases += 1;
    }
    let targets = [
        ("0", Some(0)),
        ("+0", Some(0)),
        ("-0", Some(0)),
        ("000001", Some(1)),
        ("+2147483647", Some(i32::MAX)),
        ("-2147483648", Some(i32::MIN)),
        ("2147483648", None),
        ("-2147483649", None),
        ("18446744073709551615", None),
        ("", None),
        ("+", None),
        ("-", None),
        (" 1", None),
        ("1 ", None),
        ("1_000", None),
        ("0x10", None),
        ("١", None),
        ("１２", None),
        ("1\0", None),
        ("1é🙂", None),
        ("\n1", None),
    ];
    let mut raw_errors = Vec::new();
    for (raw, expected) in targets {
        let input = expression(QueryExpressionKind::TargetLiteral(CompactString::new(raw)));
        let before = format!("{input:?}");
        let QueryExpressionKind::TargetLiteral(owner) = &input.kind else {
            unreachable!()
        };
        let pointer = owner.as_ptr();
        let capacity = owner.capacity();
        let result = input.java_integer_literal();
        match (result, expected) {
            (Ok(actual), Some(expected)) => assert_eq!(actual, expected),
            (Err(actual), None) => {
                assert_eq!(actual, raw);
                assert_eq!(actual.capacity(), owner.clone().capacity());
                raw_errors.push(actual.to_string());
            }
            (actual, expected) => panic!("{raw:?}: {actual:?} != {expected:?}"),
        }
        assert_eq!(owner.as_ptr(), pointer);
        assert_eq!(owner.capacity(), capacity);
        assert_eq!(format!("{input:?}"), before);
    }
    let fallback_sources = [
        "deps(//:x)",
        "let x = //:x in $x",
        "set(//:x //:y)",
        "a union b",
    ];
    for source in fallback_sources {
        let input = QueryExpression::parse(source).unwrap();
        let before = format!("{input:?}");
        let expected = CompactString::from(input.to_string());
        let actual = input.java_integer_literal().unwrap_err();
        assert_eq!(actual, expected);
        assert_eq!(actual.capacity(), expected.capacity());
        assert_eq!(format!("{input:?}"), before);
        raw_errors.push(actual.to_string());
    }
    // A public AST can contain an empty binary sequence whose display is just its left operand.
    for (value, expected) in [(123, Ok(123)), (u64::MAX, Err(u64::MAX.to_string()))] {
        let input = expression(QueryExpressionKind::BinaryOpSequence {
            left: Box::new(expression(QueryExpressionKind::Integer(value))),
            operations: Arc::from([]),
        });
        let result = input.java_integer_literal().map_err(|raw| raw.to_string());
        assert_eq!(result, expected);
    }
    println!(
        "QUERY_INTEGER_CASES={numeric_cases} TARGET_CASES={} FALLBACK_CASES={}",
        targets.len(),
        fallback_sources.len()
    );
    println!("QUERY_INTEGER_RAW_ERRORS={raw_errors:?}");
    for spare in [false, true] {
        let raw = "invalid é🙂 integer ".repeat(2048);
        let mut owner = CompactString::with_capacity(raw.len() + if spare { 16384 } else { 0 });
        owner.push_str(&raw);
        let expected_capacity = owner.clone().capacity();
        let input = expression(QueryExpressionKind::TargetLiteral(owner));
        let error = input.java_integer_literal().unwrap_err();
        let cloned = error.clone();
        drop(input);
        assert_eq!(error, raw);
        assert_eq!(error.capacity(), expected_capacity);
        drop(error);
        assert_eq!(cloned, raw);
        drop(raw);
        assert!(cloned.ends_with(" integer "));
        println!(
            "QUERY_INTEGER_RETAINED={} CAPACITY={}",
            cloned.len(),
            cloned.capacity()
        );
    }
}

#[test]
fn integer_slot_public_validators_preserve_diagnostics_spans_and_signed_semantics() {
    for source in [
        "some(//:x, 0)",
        "some(//:x, 2147483647)",
        "some(//:x, '-2147483648')",
        "some(//:x, '+0001')",
        "deps(//:x, '-1')",
        "rdeps(//..., //:x, '-2147483648')",
        "some(2147483648)",
    ] {
        validate_loading_query(&QueryExpression::parse(source).unwrap()).unwrap();
    }
    for source in [
        "some(set(//:x), '-2147483648')",
        "deps(//:x, 2147483647)",
        "deps(//:x, '+0001')",
        "rdeps(deps(//:x), //:x, '-2147483648')",
    ] {
        validate_cquery_query(&QueryExpression::parse(source).unwrap()).unwrap();
    }
    let mut errors = Vec::new();
    for (argument, raw) in [
        ("2147483648", "2147483648"),
        ("18446744073709551615", "18446744073709551615"),
        ("'-2147483649'", "-2147483649"),
        ("' 1'", " 1"),
        ("'1_000'", "1_000"),
        ("'١'", "١"),
        ("'1\0'", "1\0"),
        ("deps(//:x)", "deps(//:x)"),
    ] {
        let source = format!("some(//:x, {argument})");
        let input = QueryExpression::parse(&source).unwrap();
        for error in [
            validate_loading_query(&input).unwrap_err(),
            validate_cquery_query(&input).unwrap_err(),
        ] {
            assert_eq!(
                error.message,
                format!("expected an integer literal: '{raw}'")
            );
            assert_eq!(
                error.span,
                SourceSpan {
                    start: 11,
                    end: 11 + argument.len()
                }
            );
            errors.push(error.to_string());
        }
    }
    let input = QueryExpression::parse("deps(//:x, '-1')").unwrap();
    let error = validate_cquery_query(&input).unwrap_err();
    assert_eq!(error.message, "cquery deps() depth must be nonnegative");
    assert_eq!(error.span, SourceSpan { start: 11, end: 15 });
    errors.push(error.to_string());
    println!("QUERY_INTEGER_PUBLIC_ERRORS={errors:?}");
}
