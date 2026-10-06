use super::*;

#[test]
fn negative_context_tokens_preserve_exact_scanning_and_three_token_limit() {
    let cases = [
        ("(-1)", "- 1 )"),
        ("( \t\r\n-1)", "- 1 )"),
        ("(-1", "- 1"),
        ("(-1   ", "- 1"),
        ("(-1,2)", "- 1 ,"),
        ("(-1+2)", "- 1 +"),
        ("(-1=2)", "- 1 ="),
        ("(-1^2)", "- 1 ^"),
        ("(-1-2)", "- 1-2 )"),
        ("(-1foo)", "- 1foo )"),
        ("(-1é🙂)", "- 1é🙂 )"),
        ("(-1\u{2003}x)", "- 1\u{2003}x )"),
        ("(-1 é🙂)", "- 1 é🙂"),
        ("(-1~[abc]/@._:$*)", "- 1~[abc]/@._:$* )"),
        ("(-1 #foo)", "- 1 #foo"),
        ("(-1 %foo)", "- 1 %foo"),
        ("(-1 \0x)", "- 1 \0x"),
        ("(-1 @@repo++ext//:x)", "- 1 @@repo++ext//:x"),
        ("(-1 @repo+ext//:x)", "- 1 @repo"),
        ("(-1 'é 🙂,(-2)')", "- 1 é 🙂,(-2)"),
        ("(-1 \"é 🙂,(-2)\")", "- 1 é 🙂,(-2)"),
        ("(-1 '')", "- 1 "),
        ("(-1 \"\")", "- 1 "),
        ("(-1 'unterminated🙂", "- 1 unterminated🙂"),
        ("(-1 \"unterminated🙂", "- 1 unterminated🙂"),
        ("(-1 '", "- 1 "),
        ("(-1 \"", "- 1 "),
        ("(1, -2, -3)", "- 2 ,"),
        ("'ignored(-2)' union (-1)", "- 1 )"),
        ("\"ignored'(-2)\" union (-1)", "- 1 )"),
    ];
    for (source, expected) in cases {
        assert_eq!(
            unquoted_bare_negative_tokens(source).as_deref(),
            Some(expected),
            "{source:?}"
        );
    }
    let negatives = [
        "",
        "-1",
        "(-x)",
        "(-é)",
        "(-١)",
        "(−1)",
        "(- 1)",
        "(--1)",
        "'(-1)'",
        "\"(-1)\"",
        "'unterminated(-1",
        "(\u{2003}-1)",
    ];
    for source in negatives {
        assert_eq!(unquoted_bare_negative_tokens(source), None, "{source:?}");
    }
    println!(
        "QUERY_TOKEN_CASES={} NONE_CASES={}",
        cases.len(),
        negatives.len()
    );
}

#[test]
fn query_errors_preserve_messages_spans_priority_and_owned_lifetimes() {
    let cases = [
        ("some(-1)", "syntax error at '- 1 )'"),
        ("some(-1,2)", "syntax error at '- 1 ,'"),
        ("some(-1 'é🙂')", "syntax error at '- 1 é🙂'"),
        ("some(-1 \"\")", "syntax error at '- 1 '"),
        (
            "some(-1 @@repo++ext//:x)",
            "syntax error at '- 1 @@repo++ext//:x'",
        ),
        (
            "some(-1 \"unterminated🙂)",
            "syntax error at '- 1 unterminated🙂)'",
        ),
        ("some(-1", "premature end of input"),
        ("some(-1,", "premature end of input"),
        ("some(-1=", "premature end of input"),
        ("some(\u{2003}-1)", "syntax error in query expression"),
        ("some(- 1)", "syntax error in query expression"),
    ];
    let mut diagnostics = Vec::new();
    for (source, expected) in cases {
        let error = QueryExpression::parse(source).unwrap_err();
        assert_eq!(error.message, expected, "{source:?}");
        assert_eq!(
            error.span,
            SourceSpan {
                start: source.len(),
                end: source.len()
            }
        );
        assert_eq!(
            error.to_string(),
            format!("{expected} at bytes {}..{}", source.len(), source.len())
        );
        diagnostics.push(error.to_string());
    }
    println!("QUERY_TOKEN_ERRORS={diagnostics:?}");
    for source in [
        "some(//:single, '-1')",
        "some(//:single, \"-1\")",
        "set('(-1)' '//é:🙂')",
    ] {
        let expression = QueryExpression::parse(source).unwrap();
        assert_eq!(
            expression.span,
            SourceSpan {
                start: 0,
                end: source.len()
            }
        );
        assert!(!expression.to_string().is_empty());
    }
    for quote in ['\'', '"'] {
        let payload = "é🙂 memory owner ".repeat(2048);
        let expected = format!("syntax error at '- 1 {payload}'");
        let source = format!("some(-1 {quote}{payload}{quote})");
        let length = source.len();
        let error = QueryExpression::parse(&source).unwrap_err();
        let cloned = error.clone();
        drop(source);
        drop(payload);
        assert_eq!(error.message, expected);
        assert_eq!(cloned, error);
        assert_eq!(
            error.span,
            SourceSpan {
                start: length,
                end: length
            }
        );
        drop(error);
        assert_eq!(cloned.message, expected);
        assert_eq!(
            cloned.to_string(),
            format!("{expected} at bytes {length}..{length}")
        );
        println!("QUERY_TOKEN_RETAINED={}", cloned.message.len());
    }
}
