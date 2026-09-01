use compact_str::CompactString;
use slug_loading_v2::glob::GlobError;
use slug_loading_v2::glob::GlobSpec;
use slug_loading_v2::glob::PackageListing;
use slug_loading_v2::glob::expand_glob;

fn strings(items: &[&str]) -> Vec<CompactString> {
    items.iter().copied().map(CompactString::new).collect()
}

fn listing() -> PackageListing {
    PackageListing::new(
        strings(&["skip.txt", "sub/child.txt", "keep.txt", "BUILD.bazel"]),
        strings(&["sub", "empty"]),
        strings(&["", "sub", "subpackage"]),
        strings(&["subpackage"]),
    )
}

#[test]
fn expands_over_immutable_listing_with_sorted_results_and_excludes() {
    let spec = GlobSpec::new(["*.txt", "sub/*.txt"], ["skip.txt"], true, false).unwrap();

    assert_eq!(
        expand_glob(&listing(), &spec).unwrap(),
        vec!["keep.txt", "sub/child.txt"]
    );
}

#[test]
fn directories_are_candidates_only_when_requested() {
    let files_only = GlobSpec::new(["*"], std::iter::empty::<&str>(), true, true).unwrap();
    let with_directories = GlobSpec::new(["*"], std::iter::empty::<&str>(), false, true).unwrap();

    assert_eq!(
        expand_glob(&listing(), &files_only).unwrap(),
        vec!["BUILD.bazel", "keep.txt", "skip.txt"]
    );
    assert_eq!(
        expand_glob(&listing(), &with_directories).unwrap(),
        vec!["BUILD.bazel", "empty", "keep.txt", "skip.txt", "sub"]
    );
}

#[test]
fn allow_empty_false_is_checked_per_include_pattern() {
    let spec = GlobSpec::new(
        ["*.txt", "subpackage/*.txt"],
        std::iter::empty::<&str>(),
        true,
        false,
    )
    .unwrap();

    let error = expand_glob(&listing(), &spec).unwrap_err();
    assert_eq!(
        error,
        GlobError::EmptyPattern {
            pattern: "subpackage/*.txt".to_owned()
        }
    );
}

#[test]
fn empty_include_and_all_excluded_have_deterministic_errors() {
    let empty = GlobSpec::new(
        std::iter::empty::<&str>(),
        std::iter::empty::<&str>(),
        true,
        false,
    )
    .unwrap();
    assert_eq!(
        expand_glob(&listing(), &empty).unwrap_err(),
        GlobError::AllExcluded
    );

    let excluded = GlobSpec::new(["*.txt"], ["*.txt"], true, false).unwrap();
    assert_eq!(
        expand_glob(&listing(), &excluded).unwrap_err(),
        GlobError::AllExcluded
    );
}

#[test]
fn validates_complete_include_syntax_and_admits_recursive_and_literal_punctuation() {
    for pattern in ["", "/abs", "a/", "a//b", "a/**x", "a?", ".", "..", "a/../b"] {
        assert!(
            matches!(
                GlobSpec::new([pattern], std::iter::empty::<&str>(), true, true),
                Err(GlobError::InvalidPattern { .. })
            ),
            "{pattern:?} should be rejected"
        );
    }

    let listing = PackageListing::new(
        strings(&[
            "root.txt",
            "a/one.txt",
            "a/b/two.txt",
            "a/[x]",
            "a/{x}",
            r"a/x\y",
            "a/(literal)",
            "a/value.txt",
        ]),
        strings(&["a", "a/b"]),
        strings(&["", "a", "a/b"]),
        vec![],
    );
    let spec = GlobSpec::new(
        [
            "**/*.txt",
            "a/**/two.txt",
            "a/[x]",
            "a/{x}",
            r"a/x\y",
            "a/(literal)",
        ],
        std::iter::empty::<&str>(),
        true,
        false,
    )
    .unwrap();
    assert_eq!(
        expand_glob(&listing, &spec).unwrap(),
        [
            "a/(literal)",
            "a/[x]",
            "a/b/two.txt",
            "a/one.txt",
            "a/value.txt",
            r"a/x\y",
            "a/{x}",
            "root.txt",
        ]
    );

    let trailing = GlobSpec::new(["a/**"], std::iter::empty::<&str>(), false, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &trailing).unwrap(),
        [
            "a",
            "a/(literal)",
            "a/[x]",
            "a/b",
            "a/b/two.txt",
            "a/one.txt",
            "a/value.txt",
            r"a/x\y",
            "a/{x}",
        ]
    );
}

#[test]
fn matcher_preserves_hidden_and_wildcard_parenthesis_rules() {
    let listing = PackageListing::new(
        strings(&[".hidden.txt", "plain.txt", "(plain).txt"]),
        vec![],
        strings(&[""]),
        vec![],
    );
    let all = GlobSpec::new(["*"], std::iter::empty::<&str>(), true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &all).unwrap(),
        ["(plain).txt", ".hidden.txt", "plain.txt"]
    );
    let suffix = GlobSpec::new(["*.txt"], std::iter::empty::<&str>(), true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &suffix).unwrap(),
        ["(plain).txt", "plain.txt"]
    );
    let ignored_parentheses =
        GlobSpec::new(["(*.txt)"], std::iter::empty::<&str>(), true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &ignored_parentheses).unwrap(),
        ["(plain).txt", "plain.txt"]
    );
    let literal_parentheses =
        GlobSpec::new(["(plain).txt"], std::iter::empty::<&str>(), true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &literal_parentheses).unwrap(),
        ["(plain).txt"]
    );
}

#[test]
fn exclusions_preserve_bazel_shortcuts_question_and_error_precedence() {
    let listing = PackageListing::new(
        strings(&["a.txt", "é.txt", "keep.txt", "prefix-tail"]),
        vec![],
        strings(&[""]),
        vec![],
    );
    let question = GlobSpec::new(["*"], ["?.txt"], true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &question).unwrap(),
        ["keep.txt", "prefix-tail"]
    );

    let inert_literals = GlobSpec::new(
        ["keep.txt"],
        ["", "/absolute", ".", "..", "bad//path"],
        true,
        false,
    )
    .unwrap();
    assert_eq!(
        expand_glob(&listing, &inert_literals).unwrap(),
        ["keep.txt"]
    );

    let shortcut = GlobSpec::new(["*"], ["pre**/*tail"], true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &shortcut).unwrap(),
        ["a.txt", "keep.txt", "é.txt"]
    );

    let invalid = GlobSpec::new(["keep.txt"], ["bad/**x*"], true, false).unwrap();
    assert!(matches!(
        expand_glob(&listing, &invalid),
        Err(GlobError::InvalidPattern { .. })
    ));
    let empty_first = GlobSpec::new(["missing"], ["bad/**x*"], true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &empty_first).unwrap_err(),
        GlobError::EmptyPattern {
            pattern: "missing".to_owned()
        }
    );
    let no_includes = GlobSpec::new(std::iter::empty::<&str>(), ["bad/**x*"], true, true).unwrap();
    assert!(matches!(
        expand_glob(&listing, &no_includes),
        Err(GlobError::InvalidPattern { .. })
    ));
}

#[test]
fn projection_escapes_leading_at_and_uses_java_utf16_order() {
    let listing = PackageListing::new(
        strings(&["@lead", "\u{e000}", "\u{10000}"]),
        vec![],
        strings(&[""]),
        vec![],
    );
    let spec = GlobSpec::new(["*"], std::iter::empty::<&str>(), true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &spec).unwrap(),
        [":@lead", "\u{10000}", "\u{e000}"]
    );
}
