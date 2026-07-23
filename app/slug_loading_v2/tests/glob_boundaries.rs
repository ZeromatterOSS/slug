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
fn rejects_patterns_outside_the_reviewed_subset() {
    for pattern in [
        "", "/abs", "a/", "a//b", r"a\b", "**", "a/**", "a?", "[ab]", "{a,b}", ".", "..", "a/../b",
    ] {
        assert!(
            matches!(
                GlobSpec::new([pattern], std::iter::empty::<&str>(), true, true),
                Err(GlobError::InvalidPattern { .. })
            ),
            "{pattern:?} should be rejected"
        );
    }
}
