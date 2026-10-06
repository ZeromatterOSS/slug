use super::*;

fn original_expand(listing: &PackageListing, spec: &GlobSpec) -> Result<Vec<String>, GlobError> {
    let mut include_matched = vec![false; spec.includes.len()];
    let mut matches = Vec::new();
    let candidates = listing.regular_files.iter().chain(
        (!spec.exclude_directories)
            .then_some(listing.directories.iter())
            .into_iter()
            .flatten(),
    );
    for candidate in candidates {
        let mut included = false;
        for (index, pattern) in spec.includes.iter().enumerate() {
            if pattern.matches_bytes(candidate.as_bytes()) {
                include_matched[index] = true;
                included = true;
            }
        }
        if included {
            matches.push(candidate.to_string());
        }
    }
    spec.check_include_matches(&include_matched)?;
    spec.validate_excludes()?;
    matches.retain(|candidate| !spec.is_excluded(candidate.as_bytes()));
    spec.check_final_matches(matches.is_empty())?;
    for path in &mut matches {
        if path.starts_with('@') {
            path.insert(0, ':');
        }
    }
    matches.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
    matches.dedup();
    Ok(matches)
}

#[test]
fn excluded_paths_leave_no_final_vector_backing_and_mixed_outputs_remain_owned() {
    let paths = (0..128)
        .map(|index| CompactString::from(format!("skip_{index}_{}.txt", "é🙂".repeat(256))))
        .collect::<Vec<_>>();
    let listing = PackageListing::new(paths.clone(), vec![], vec![], vec![]);
    let clone = listing.clone();
    let before = listing
        .regular_files
        .iter()
        .map(|path| (path.to_string(), path.capacity(), path.as_ptr()))
        .collect::<Vec<_>>();
    let spec = GlobSpec::new(["*"], ["skip_**/*txt"], true, true).unwrap();
    let expected = original_expand(&listing, &spec).unwrap();
    let actual = expand_glob(&listing, &spec).unwrap();
    assert_eq!(actual, expected);
    assert!(actual.is_empty());
    let capacity = actual.capacity();
    println!(
        "GLOB_EXCLUDED_COUNT={} CAPACITY={capacity}",
        listing.regular_files.len()
    );
    for (path, (bytes, capacity, pointer)) in listing.regular_files.iter().zip(&before) {
        assert_eq!(path.as_str(), bytes);
        assert_eq!(path.capacity(), *capacity);
        assert_eq!(path.as_ptr(), *pointer);
    }
    let mut mixed_paths = paths.clone();
    let long = format!("keep_{}", "é🙂".repeat(512));
    mixed_paths.extend([
        CompactString::from(long.clone()),
        CompactString::from("@keep"),
        CompactString::from("keep"),
    ]);
    let mixed = PackageListing::new(mixed_paths, vec![], vec![], vec![]);
    let held = expand_glob(&mixed, &spec).unwrap();
    assert_eq!(held, original_expand(&mixed, &spec).unwrap());
    drop(mixed);
    drop(listing);
    drop(clone);
    drop(spec);
    drop(paths);
    assert_eq!(held, [":@keep", "keep", long.as_str()]);
    assert_eq!(held[2].len(), 3077);
    drop(long);
    assert_eq!(held[2], format!("keep_{}", "é🙂".repeat(512)));
    println!(
        "GLOB_MIXED_RETAINED={:?}",
        held.iter().map(String::len).collect::<Vec<_>>()
    );
    // The baseline retains backing storage after removing all copied matches.
    assert_eq!(capacity, 0);
}

#[test]
fn exclusion_scheduling_preserves_full_results_error_priority_and_projection() {
    let paths = [
        "a.txt",
        "é.txt",
        "🙂.txt",
        "@lead",
        ":@lead",
        ".hidden.txt",
        "(plain).txt",
        "a/(literal)",
        "a/[x]",
        "a/one.txt",
        "a/b/two.txt",
        "prefix-tail",
        "nul\0name",
        "\u{e000}",
        "\u{10000}",
    ];
    let listing = PackageListing::new(
        paths.map(CompactString::from).to_vec(),
        vec![
            CompactString::from("a"),
            CompactString::from("a/b"),
            CompactString::from("a.txt"),
        ],
        vec![],
        vec![],
    );
    let includes: &[&[&str]] = &[
        &[],
        &["*"],
        &["*.txt"],
        &["**/*.txt"],
        &["a/**"],
        &["missing"],
        &["*.txt", "missing"],
        &["*", "*", "a/**"],
        &["(plain).txt"],
    ];
    let excludes: &[&[&str]] = &[
        &[],
        &["*.txt"],
        &["?.txt"],
        &["**"],
        &["pre**/*tail"],
        &["@lead"],
        &[":@lead"],
        &["bad/**x*"],
        &["*.txt", "bad/**x*"],
        &["bad/**x*", "?.txt"],
        &["", "/absolute", ".", "..", "bad//path"],
        &["a/**"],
    ];
    let before = listing.clone();
    let mut comparisons = 0;
    let mut errors = Vec::new();
    for include in includes {
        for exclude in excludes {
            for directories in [true, false] {
                for empty in [true, false] {
                    let spec = GlobSpec::new(
                        include.iter().copied(),
                        exclude.iter().copied(),
                        directories,
                        empty,
                    )
                    .unwrap();
                    let expected = original_expand(&listing, &spec);
                    let actual = expand_glob(&listing, &spec);
                    assert_eq!(
                        actual, expected,
                        "include={include:?} exclude={exclude:?} directories={directories} allow_empty={empty}"
                    );
                    if let Err(error) = actual {
                        errors.push(error.to_string());
                    }
                    comparisons += 1;
                }
            }
        }
    }
    let empty_first = GlobSpec::new(["*.txt", "missing"], ["**", "bad/**x*"], true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &empty_first).unwrap_err(),
        GlobError::EmptyPattern {
            pattern: "missing".to_owned()
        }
    );
    let invalid_before_final = GlobSpec::new(["*"], ["**", "bad/**x*"], true, false).unwrap();
    assert!(matches!(
        expand_glob(&listing, &invalid_before_final),
        Err(GlobError::InvalidPattern { .. })
    ));
    let all_excluded = GlobSpec::new(["*"], ["**"], true, false).unwrap();
    assert_eq!(
        expand_glob(&listing, &all_excluded).unwrap_err(),
        GlobError::AllExcluded
    );
    let pre_escape = GlobSpec::new(["*"], [":@lead"], true, true).unwrap();
    let output = expand_glob(&listing, &pre_escape).unwrap();
    assert!(output.iter().any(|path| path == ":@lead"));
    let escape_dedup = GlobSpec::new(["*"], std::iter::empty::<&str>(), false, true).unwrap();
    let output = expand_glob(&listing, &escape_dedup).unwrap();
    assert_eq!(
        output
            .iter()
            .filter(|path| path.as_str() == ":@lead")
            .count(),
        1
    );
    assert_eq!(
        output
            .iter()
            .filter(|path| path.as_str() == "a.txt")
            .count(),
        1
    );
    assert!(
        output.iter().position(|path| path == "\u{10000}").unwrap()
            < output.iter().position(|path| path == "\u{e000}").unwrap()
    );
    assert_eq!(listing, before);
    println!("GLOB_EXCLUDE_COMPARISONS={comparisons}");
    println!("GLOB_EXCLUDE_ERRORS={errors:?}");
}
