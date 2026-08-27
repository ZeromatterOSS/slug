use slug_identity_v2::TargetPattern;
use slug_identity_v2::TargetPatternWildcard;

#[test]
fn parses_target_patterns() {
    let cases = [
        ("//pkg:target", "//pkg:target"),
        ("//pkg", "//pkg:pkg"),
        ("//pkg:all", "//pkg:all"),
        ("//pkg:*", "//pkg:*"),
        ("//pkg:all-targets", "//pkg:all-targets"),
        ("//...", "//..."),
        ("//...:all", "//...:all"),
        ("//...:*", "//...:*"),
        ("//...:all-targets", "//...:all-targets"),
        ("//pkg/...", "//pkg/..."),
        ("//pkg/...:all", "//pkg/...:all"),
        ("//pkg/...:*", "//pkg/...:*"),
        ("//pkg/...:all-targets", "//pkg/...:all-targets"),
        ("@repo//pkg:target", "@repo//pkg:target"),
        ("@repo//pkg:all", "@repo//pkg:all"),
        ("@repo//pkg:*", "@repo//pkg:*"),
        ("@repo//pkg:all-targets", "@repo//pkg:all-targets"),
        ("@repo//...", "@repo//..."),
        ("@repo//pkg/...", "@repo//pkg/..."),
        ("@repo//pkg/...:all-targets", "@repo//pkg/...:all-targets"),
    ];
    for (raw, display) in cases {
        assert_eq!(TargetPattern::parse(raw).unwrap().to_string(), display);
    }
}

// Bazel 9.2 TargetPattern.Parser and TargetPatternTest retain these suffixes
// before package lookup resolves an absolute wildcard-name conflict.
#[test]
fn retains_package_wildcard_spelling_and_recursive_policy() {
    for (raw, expected, rules_only) in [
        ("//pkg:all", TargetPatternWildcard::All, true),
        ("//pkg:*", TargetPatternWildcard::Star, false),
        (
            "//pkg:all-targets",
            TargetPatternWildcard::AllTargets,
            false,
        ),
    ] {
        let TargetPattern::PackageWildcard { wildcard, .. } = TargetPattern::parse(raw).unwrap()
        else {
            panic!("{raw} must remain an unresolved package wildcard")
        };
        assert_eq!(wildcard, expected);
        assert_eq!(wildcard.rules_only(), rules_only);
    }

    for (raw, expected, rules_only) in [
        ("//pkg/...", None, true),
        ("//pkg/...:all", Some(TargetPatternWildcard::All), true),
        ("//pkg/...:*", Some(TargetPatternWildcard::Star), false),
        (
            "//pkg/...:all-targets",
            Some(TargetPatternWildcard::AllTargets),
            false,
        ),
    ] {
        let TargetPattern::Recursive { wildcard, .. } = TargetPattern::parse(raw).unwrap() else {
            panic!("{raw} must remain recursive syntax")
        };
        assert_eq!(wildcard, expected);
        assert_eq!(
            wildcard.is_none_or(TargetPatternWildcard::rules_only),
            rules_only
        );
    }
}

#[test]
fn rejects_non_wildcard_recursive_targets() {
    for raw in ["//...:target", "//pkg/...:target", "@repo//pkg/...:target"] {
        assert!(TargetPattern::parse(raw).is_err(), "{raw}");
    }
}

#[test]
fn rejects_canonical_target_patterns_at_apparent_boundary() {
    assert!(TargetPattern::parse("@@repo//pkg:target").is_err());
}
