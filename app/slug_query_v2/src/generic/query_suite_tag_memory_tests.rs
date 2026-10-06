use std::hash::Hash;
use std::hash::Hasher;

use super::*;

fn original_split(tags: &[CompactString]) -> (SmallSet<CompactString>, SmallSet<CompactString>) {
    let mut required = SmallSet::new();
    let mut excluded = SmallSet::new();
    for tag in tags {
        if let Some(tag) = tag.strip_prefix('-') {
            excluded.insert(CompactString::new(tag));
        } else if let Some(tag) = tag.strip_prefix('+') {
            required.insert(CompactString::new(tag));
        } else if tag != "manual" {
            required.insert(tag.clone());
        }
    }
    (required, excluded)
}

fn hash<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut state);
    state.finish()
}

#[test]
fn split_tags_borrow_first_source_owners_on_small_and_indexed_sets() {
    let mut observed = Vec::new();
    for count in [1, 40] {
        let mut tags = Vec::new();
        for index in 0..count {
            let ordinary = format!("plain_{index}_{}", "é🙂\0".repeat(128));
            let positive = format!("+plus_{index}_{}", "é🙂\0".repeat(128));
            let negative = format!("-minus_{index}_{}", "é🙂\0".repeat(128));
            tags.extend([ordinary, positive, negative].map(CompactString::from));
        }
        // Independently owned duplicates must retain the first equal key.
        tags.extend(tags.clone());
        tags.push(CompactString::from("manual"));
        let source = tags
            .iter()
            .map(|tag| (tag.to_string(), tag.capacity(), tag.as_ptr()))
            .collect::<Vec<_>>();
        let (expected_required, expected_excluded) = original_split(&tags);
        let (required, excluded) = split_test_tags(&tags);
        assert_eq!(required.len(), count * 2);
        assert_eq!(excluded.len(), count);
        for (actual, expected, offset, slot) in [
            (&required, &expected_required, 0usize, 0usize),
            (&excluded, &expected_excluded, 1, 2),
        ] {
            for (index, (tag, expected)) in actual.iter().zip(expected.iter()).enumerate() {
                let raw: &str = tag.as_ref();
                assert_eq!(raw, expected.as_str());
                assert_eq!(hash(tag), hash(expected));
                assert!(actual.contains(raw));
                let source_index = if slot == 0 {
                    index / 2 * 3 + index % 2
                } else {
                    index * 3 + slot
                };
                let prefix = if slot == 0 { index % 2 } else { offset };
                let owner = &tags[source_index].as_str()[prefix..];
                assert_eq!(raw, owner);
                observed.push(raw.as_ptr() == owner.as_ptr());
            }
        }
        assert!(!required.contains("manual"));
        for (tag, (bytes, capacity, pointer)) in tags.iter().zip(&source) {
            assert_eq!(tag.as_str(), bytes);
            assert_eq!(tag.capacity(), *capacity);
            assert_eq!(tag.as_ptr(), *pointer);
        }
    }
    println!("SUITE_TAG_SHARING={observed:?}");
    // Record every observation before the unchanged baseline's intended failure.
    assert_eq!(observed, vec![true; 123]);
}

#[test]
fn tag_prefix_manual_conflicts_and_size_filtering_preserve_original_semantics() {
    let cases: &[(&[&str], &[&str], Option<&str>, bool)] = &[
        (&[], &[], None, true),
        (&["manual"], &[], None, true),
        (&["+manual"], &[], None, false),
        (&["+manual"], &["manual"], None, true),
        (&["-manual"], &["manual"], None, false),
        (&["+", "+", ""], &[""], None, true),
        (&["-"], &[""], None, false),
        (&["++a"], &["+a"], None, true),
        (&["--a"], &["-a"], None, false),
        (&["a", "-a"], &["a"], None, false),
        (&["large"], &[], Some("large"), true),
        (&["-large"], &[], Some("large"), false),
        (&["a", "+a"], &["a", "a"], None, true),
        (&["é🙂\0", "-missing"], &["é🙂\0"], None, true),
    ];
    let mut comparisons = 0;
    let mut retained = Vec::new();
    for (suite, tags, size, expected) in cases {
        let source = suite
            .iter()
            .map(|s| CompactString::from(*s))
            .collect::<Vec<_>>();
        let info = TestTargetInfo {
            label: CompactString::from("//suite:member"),
            kind: TestTargetKind::Test,
            tags: tags
                .iter()
                .map(|s| CompactString::from(*s))
                .collect::<Vec<_>>()
                .into(),
            size: size.map(CompactString::from),
        };
        let (required, excluded) = split_test_tags(&source);
        assert_eq!(include_test(&info, &required, &excluded), *expected);
        retained.push(include_test(&info, &required, &excluded));
        comparisons += 1;
    }
    let alphabet = [
        "", "manual", "+manual", "-manual", "+", "-", "++a", "--a", "a", "-a", "é🙂\0",
    ];
    for left in alphabet {
        for right in alphabet {
            let source = [
                CompactString::from(left),
                CompactString::from(right),
                CompactString::from(left),
            ];
            let snapshot = source.clone();
            let (original_required, original_excluded) = original_split(&source);
            let (required, excluded) = split_test_tags(&source);
            for tag in alphabet {
                for size in [None, Some(""), Some("a"), Some("manual"), Some("é🙂\0")] {
                    let info = TestTargetInfo {
                        label: CompactString::from("//suite:member"),
                        kind: TestTargetKind::Test,
                        tags: vec![CompactString::from(tag)].into(),
                        size: size.map(CompactString::from),
                    };
                    let has_tag = |tag: &CompactString| {
                        info.tags.iter().any(|candidate| candidate == tag)
                            || info.size.as_ref() == Some(tag)
                    };
                    let expected = original_excluded.iter().all(|tag| !has_tag(tag))
                        && original_required.iter().all(has_tag);
                    assert_eq!(
                        include_test(&info, &required, &excluded),
                        expected,
                        "suite={source:?}, tags={:?}, size={:?}",
                        info.tags,
                        info.size
                    );
                    comparisons += 1;
                }
            }
            assert_eq!(source, snapshot);
        }
    }
    assert_eq!(
        retained,
        cases.iter().map(|case| case.3).collect::<Vec<_>>()
    );
    println!("SUITE_TAG_COMPARISONS={comparisons}");
    println!("SUITE_TAG_RETAINED={retained:?}");
}
