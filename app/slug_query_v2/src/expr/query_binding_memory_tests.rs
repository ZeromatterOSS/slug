use super::*;

fn binding_snapshot<T: AsRef<str>>(bindings: &SmallSet<T>) -> Vec<(String, *const u8)> {
    bindings
        .iter()
        .map(|name| (name.as_ref().to_owned(), name.as_ref().as_ptr()))
        .collect()
}

fn name_snapshot(input: &QueryExpression) -> Vec<(String, *const u8, usize)> {
    fn visit(input: &QueryExpression, names: &mut Vec<(String, *const u8, usize)>) {
        match &input.kind {
            QueryExpressionKind::Let { name, value, body } => {
                names.push((
                    name.value.to_string(),
                    name.value.as_ptr(),
                    name.value.capacity(),
                ));
                visit(value, names);
                visit(body, names);
            }
            QueryExpressionKind::BinaryOpSequence { left, operations } => {
                visit(left, names);
                for (_, right) in operations.iter() {
                    visit(right, names);
                }
            }
            QueryExpressionKind::Function { args, .. } => {
                for argument in args.iter() {
                    visit(argument, names);
                }
            }
            _ => {}
        }
    }
    let mut names = Vec::new();
    visit(input, &mut names);
    names
}

#[test]
fn cquery_binding_scratch_preserves_shadowing_cleanup_seed_owners_and_order() {
    let cases = [
        ("let x = //:a in $x", None),
        ("let x = $external in (let x = $x in $x) union $x", None),
        (
            "(let x = //:a in $x) union $x",
            Some("undefined query variable '$x'"),
        ),
        ("let x = $x in $x", Some("undefined query variable '$x'")),
        ("let external = $external in $external", None),
        (
            "let external = //:new in $missing",
            Some("undefined query variable '$missing'"),
        ),
        (
            "let x = //:a in let y = $x in $missing",
            Some("undefined query variable '$missing'"),
        ),
        (
            "let x = //:a in some($x, nope)",
            Some("expected an integer literal: 'nope'"),
        ),
        (
            "let x = //:a in set($x)",
            Some("set() accepts only concrete target literals"),
        ),
        ("let x = //:a in visible($x, $external)", None),
        (
            "let x = //:a in kind('rule', executables(siblings($x)))",
            None,
        ),
        (
            "let x = //:a in filter('x', buildfiles(loadfiles(some($x, 1))))",
            None,
        ),
        (
            "let x = //:a in deps($x)",
            Some("deps() is supported only as a top-level cquery expression"),
        ),
    ];
    let mut errors = Vec::new();
    let external = format!("external_{}", "owner".repeat(128));
    for seed_count in [1, 40] {
        let seeds = std::iter::once(external.clone())
            .chain((1..seed_count).map(|i| format!("seed_{i}_{}", "owner".repeat(128))))
            .collect::<Vec<_>>();
        for (source, expected) in cases {
            let source = source.replace("external", &external);
            let input = QueryExpression::parse(&source).unwrap();
            let debug = format!("{input:?}");
            let names = name_snapshot(&input);
            // Inference permits identical tests for owned and borrowed private scratch keys.
            let mut bindings = seeds
                .iter()
                .map(|name| name.as_str().into())
                .collect::<SmallSet<_>>();
            let before = binding_snapshot(&bindings);
            let result = validate_cquery_query_inner(&input, &mut bindings);
            match (result, expected) {
                (Ok(()), None) => {}
                (Err(error), Some(expected)) => {
                    assert_eq!(error.message, expected, "{source}");
                    errors.push(error.to_string());
                }
                (actual, expected) => panic!("{source}: {actual:?} != {expected:?}"),
            }
            assert_eq!(binding_snapshot(&bindings), before, "{source}");
            assert_eq!(name_snapshot(&input), names);
            assert_eq!(format!("{input:?}"), debug);
        }
    }
    println!("QUERY_BINDING_DIRECT_CASES={}", cases.len() * 2);
    println!("QUERY_BINDING_DIRECT_ERRORS={errors:?}");
}

#[test]
fn cquery_long_names_preserve_public_errors_spans_and_retained_ownership() {
    let name = format!("scope_{}", "owner".repeat(1024));
    let successes = [
        format!("let {name} = '//é:🙂' in ${name}"),
        format!("let {name} = //:a in (let {name} = ${name} in ${name}) union ${name}"),
        format!("let {name} = //:a in some(filter('é', ${name}), 1)"),
        format!("visible(let {name} = //:a in ${name}, let {name} = //:b in ${name})"),
    ];
    for source in &successes {
        let input = QueryExpression::parse(source).unwrap();
        let debug = format!("{input:?}");
        let owners = name_snapshot(&input);
        validate_cquery_query(&input).unwrap();
        validate_loading_query(&input).unwrap();
        assert_eq!(name_snapshot(&input), owners);
        assert_eq!(format!("{input:?}"), debug);
    }
    let sources = [
        format!("let {name} = ${name} in ${name}"),
        format!("(let {name} = //:a in ${name}) union ${name}"),
        format!("let {name} = //:a in $missing"),
        format!("let {name} = //:a in let {name} = ${name} in $missing"),
    ];
    let mut errors = Vec::new();
    for source in &sources {
        let input = QueryExpression::parse(source).unwrap();
        let owners = name_snapshot(&input);
        let error = validate_cquery_query(&input).unwrap_err();
        let expected_name = if source.ends_with("$missing") {
            "missing"
        } else {
            &name
        };
        assert_eq!(
            error.message,
            format!("undefined query variable '${expected_name}'")
        );
        let start = if source.starts_with("let") && !source.ends_with("$missing") {
            source.find(&format!("${name}")).unwrap()
        } else {
            source.rfind('$').unwrap()
        };
        assert_eq!(
            error.span,
            SourceSpan {
                start,
                end: start + 1 + expected_name.len()
            }
        );
        assert_eq!(name_snapshot(&input), owners);
        // Record compact failures separately from the long owner lifetime discriminator.
        if expected_name == "missing" {
            errors.push(error.to_string());
        }
    }
    let source = format!("(let {name} = //:a in ${name}) union ${name}");
    let input = QueryExpression::parse(&source).unwrap();
    let error = validate_cquery_query(&input).unwrap_err();
    let expected = error.to_string();
    let cloned = error.clone();
    drop(source);
    drop(input);
    drop(name);
    assert_eq!(error.to_string(), expected);
    drop(error);
    assert_eq!(cloned.to_string(), expected);
    for source in ["let x = //:a in $x", "let x = //:b in $x"] {
        validate_cquery_query(&QueryExpression::parse(source).unwrap()).unwrap();
    }
    println!("QUERY_BINDING_PUBLIC_ERRORS={errors:?}");
    println!(
        "QUERY_BINDING_RETAINED={} CAPACITY={}",
        cloned.message.len(),
        cloned.message.capacity()
    );
}
