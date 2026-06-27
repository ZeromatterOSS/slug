use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
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
