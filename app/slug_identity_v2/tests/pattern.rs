use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::CanonicalTargetPattern;
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

#[test]
fn contextually_projects_current_mapped_and_canonical_patterns() {
    let current = CanonicalRepoName::new("owner+").unwrap();
    let mapped = CanonicalRepoName::new("visible+").unwrap();
    let resolve = |apparent: &ApparentRepoName| (apparent.as_str() == "visible").then_some(&mapped);

    let CanonicalTargetPattern::Single(current_target) =
        CanonicalTargetPattern::parse("//pkg:target", &current, resolve).unwrap()
    else {
        panic!("current-repository target must remain exact")
    };
    assert_eq!(current_target.to_string(), "@@owner+//pkg:target");

    let CanonicalTargetPattern::PackageWildcard {
        package,
        wildcard,
        conflict_target,
    } = CanonicalTargetPattern::parse("@visible//pkg:all", &current, resolve).unwrap()
    else {
        panic!("mapped package wildcard must retain its shape")
    };
    assert_eq!(package.to_string(), "@@visible+//pkg");
    assert_eq!(wildcard, TargetPatternWildcard::All);
    assert_eq!(conflict_target.unwrap().to_string(), "@@visible+//pkg:all");

    let CanonicalTargetPattern::Recursive { package, wildcard } =
        CanonicalTargetPattern::parse("@@canonical+//deep/...:*", &current, resolve).unwrap()
    else {
        panic!("canonical recursive pattern must retain its shape")
    };
    assert_eq!(package.to_string(), "@@canonical+//deep");
    assert_eq!(wildcard, Some(TargetPatternWildcard::Star));
}

#[test]
fn explicit_empty_apparent_repo_requires_its_mapping_entry() {
    let root = CanonicalRepoName::root();
    let nonroot = CanonicalRepoName::new("owner+").unwrap();
    let mapped_root = CanonicalRepoName::root();

    let CanonicalTargetPattern::Single(label) =
        CanonicalTargetPattern::parse("@//pkg:target", &root, |apparent| {
            apparent.is_root().then_some(&mapped_root)
        })
        .unwrap()
    else {
        panic!("root empty apparent mapping must produce one exact target")
    };
    assert_eq!(label.to_string(), "@@//pkg:target");

    let error = CanonicalTargetPattern::parse("@//pkg:target", &nonroot, |_| None).unwrap_err();
    assert!(error.contains("repository '@' is not visible from @@owner+"));
    assert_eq!(
        CanonicalTargetPattern::parse("//pkg:target", &nonroot, |_| None).unwrap(),
        CanonicalTargetPattern::parse("@@owner+//pkg:target", &root, |_| None).unwrap()
    );
}
