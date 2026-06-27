use slug_identity_v2::TargetPattern;

#[test]
fn parses_target_patterns() {
    let cases = [
        ("//pkg:target", "//pkg:target"),
        ("//pkg", "//pkg:pkg"),
        ("//pkg:all", "//pkg:all"),
        ("//pkg/...", "//pkg/..."),
        ("@repo//pkg:target", "@repo//pkg:target"),
        ("@repo//pkg:all", "@repo//pkg:all"),
        ("@repo//pkg/...", "@repo//pkg/..."),
    ];
    for (raw, display) in cases {
        assert_eq!(TargetPattern::parse(raw).unwrap().to_string(), display);
    }
}

#[test]
fn rejects_canonical_target_patterns_at_apparent_boundary() {
    assert!(TargetPattern::parse("@@repo//pkg:target").is_err());
}
