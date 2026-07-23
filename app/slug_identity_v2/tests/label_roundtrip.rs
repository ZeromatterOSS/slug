use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_identity_v2::TargetName;
use slug_identity_v2::serialization::StableSerialize;

#[test]
fn apparent_labels_roundtrip_many_examples() {
    let labels = [
        "//foo:foo",
        "//foo/bar:baz",
        "//foo/bar:file.txt",
        "//foo/bar:sub/file.txt",
        "//a:b",
        "//a/b:c",
        "//a_b:c_d",
        "//a-b:c-d",
        "//a.b:c.d",
        "//pkg:all",
        "//pkg:__pkg__",
        "//pkg:__subpackages__",
        "//pkg:target_1",
        "//pkg:target-1",
        "//pkg:target.1",
        "//pkg:target+1",
        "//pkg/sub:target",
        "//pkg/sub:target/name",
        "//pkg/sub/deep:target",
        "@repo//pkg:target",
        "@repo//pkg/sub:target",
        "@repo_name//pkg:target_name",
        "@repo-name//pkg:target-name",
        "@repo.name//pkg:target.name",
        "@repo+name//pkg:target+name",
        "@rooted//pkg:target",
        "//foo2:foo2",
        "//x0:x0",
        "//x1:x1",
        "//x2:x2",
        "//x3:x3",
        "//x4:x4",
        "//x5:x5",
        "//x6:x6",
        "//x7:x7",
        "//x8:x8",
        "//x9:x9",
        "//r0/s0:t0",
        "//r1/s1:t1",
        "//r2/s2:t2",
        "//r3/s3:t3",
        "//r4/s4:t4",
        "//r5/s5:t5",
        "//r6/s6:t6",
        "//r7/s7:t7",
        "//r8/s8:t8",
        "//r9/s9:t9",
        "@alpha//beta/gamma:delta",
        "@alpha//beta/gamma:delta/file",
        "@zeta//eta:theta",
    ];
    assert!(labels.len() >= 50);
    for label in labels {
        let parsed = ApparentLabel::parse(label).unwrap();
        assert_eq!(parsed.to_string(), label);
    }
}

#[test]
fn canonical_labels_and_mapping_serialization_distinguish_mappings() {
    let canonical = CanonicalLabel::parse("@@rules_cc//cc/toolchains:toolchain").unwrap();
    assert_eq!(canonical.to_string(), "@@rules_cc//cc/toolchains:toolchain");

    let apparent = ApparentLabel::parse("@dep//pkg:target").unwrap();
    let mut first = RepositoryMapping::new(RepositoryMappingId::new("first").unwrap());
    first.insert(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new("dep~1.0.0").unwrap(),
    );
    let mut second = RepositoryMapping::new(RepositoryMappingId::new("second").unwrap());
    second.insert(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new("dep~2.0.0").unwrap(),
    );

    let first_label = apparent.resolve(&first);
    let second_label = apparent.resolve(&second);
    assert_ne!(first_label, second_label);
    assert_ne!(
        first_label.stable_serialize(),
        second_label.stable_serialize()
    );
}

#[test]
fn rejects_invalid_labels() {
    for label in [
        "//",
        "//pkg:",
        "pkg:target",
        "@@repo//pkg:target",
        "@bad!//pkg:target",
    ] {
        assert!(ApparentLabel::parse(label).is_err(), "{label}");
    }
    assert!(CanonicalLabel::parse("@repo//pkg:target").is_err());
}

fn hash(value: &TargetName) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn target_names_match_bazel_validation_and_preserve_printable_unicode() {
    // Bazel 9.2 LabelValidator.validateTargetName accepts every printable
    // ASCII punctuation character except its structural ':' and '\\'.
    for value in [
        "name",
        "dir/name",
        ".",
        ".hidden",
        "..hidden",
        "dir/.hidden",
        "dir/..hidden",
        " !\"#$%&'()*+,-.;<=>?@[]^_`{|}~ ",
        "目标/éclair-λ.文件",
    ] {
        let parsed = TargetName::parse(value).unwrap();
        assert_eq!(parsed.as_str(), value);
        assert_eq!(parsed.to_string(), value);
    }

    for (value, expected) in [
        ("", "empty target name"),
        ("/name:bad", "target names may not start with '/'"),
        (
            "name:\r",
            "target names may not end with carriage returns (perhaps the input source is CRLF-terminated)",
        ),
        (
            "name\u{1}:bad",
            "target names may not contain non-printable characters: '\\x01'",
        ),
        (
            "name\u{7f}:bad",
            "target names may not contain non-printable characters: '\\x7F'",
        ),
        ("name:with\\slash", "target names may not contain ':'"),
        ("name\\with\u{1}", "target names may not contain '\\'"),
        (
            "dir//../name",
            "target names may not contain '//' path separators",
        ),
        (
            "./../name",
            "target names may not contain '.' as a path segment",
        ),
        (
            "dir/./../name",
            "target names may not contain '.' as a path segment",
        ),
        (
            ".././name",
            "target names may not contain up-level references '..'",
        ),
        (
            "dir/.././name",
            "target names may not contain up-level references '..'",
        ),
        (
            "dir/..",
            "target names may not contain up-level references '..'",
        ),
        ("dir/", "target names may not end with '/'"),
    ] {
        assert_eq!(TargetName::parse(value).unwrap_err(), expected, "{value:?}");
    }
}

#[test]
fn trailing_current_directory_normalizes_target_identity_and_labels() {
    let normalized = TargetName::parse("dir/name/.").unwrap();
    let direct = TargetName::parse("dir/name").unwrap();
    assert_eq!(normalized, direct);
    assert_eq!(normalized.cmp(&direct), std::cmp::Ordering::Equal);
    assert_eq!(hash(&normalized), hash(&direct));
    assert_eq!(normalized.as_str(), "dir/name");
    assert_eq!(normalized.to_string(), "dir/name");

    let apparent_normalized = ApparentLabel::parse("//pkg:dir/name/.").unwrap();
    let apparent_direct = ApparentLabel::parse("//pkg:dir/name").unwrap();
    assert_eq!(apparent_normalized, apparent_direct);
    assert_eq!(apparent_normalized.to_string(), "//pkg:dir/name");

    let canonical_normalized = CanonicalLabel::parse("@@repo//pkg:dir/name/.").unwrap();
    let canonical_direct = CanonicalLabel::parse("@@repo//pkg:dir/name").unwrap();
    assert_eq!(canonical_normalized, canonical_direct);
    assert_eq!(
        canonical_normalized.cmp(&canonical_direct),
        std::cmp::Ordering::Equal
    );
    assert_eq!(canonical_normalized.to_string(), "@@repo//pkg:dir/name");
    assert_eq!(
        canonical_normalized.stable_serialize(),
        canonical_direct.stable_serialize()
    );
    assert_eq!(
        canonical_normalized.stable_serialize(),
        "@@repo//pkg:dir/name"
    );
}

#[test]
fn labels_reject_invalid_target_name_characters_and_segments() {
    for label in [
        "//pkg:pkg:target",
        "//pkg:dir\\name",
        "//pkg:dir//name",
        "//pkg:dir/../name",
        "//pkg:dir/./name",
        "//pkg:name\u{7f}",
    ] {
        assert!(ApparentLabel::parse(label).is_err(), "{label:?}");
    }
    for label in [
        "@@repo//pkg:pkg:target",
        "@@repo//pkg:dir\\name",
        "@@repo//pkg:dir//name",
        "@@repo//pkg:dir/../name",
        "@@repo//pkg:dir/./name",
        "@@repo//pkg:name\u{7f}",
    ] {
        assert!(CanonicalLabel::parse(label).is_err(), "{label:?}");
    }
}
