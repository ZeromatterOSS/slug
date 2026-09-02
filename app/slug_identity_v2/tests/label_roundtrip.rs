use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;

use slug_identity_v2::ApparentLabel;
use slug_identity_v2::ApparentRepoName;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::CanonicalRepoName;
use slug_identity_v2::OptionLabelContext;
use slug_identity_v2::PackageIdentifier;
use slug_identity_v2::PackagePath;
use slug_identity_v2::RepositoryMapping;
use slug_identity_v2::RepositoryMappingId;
use slug_identity_v2::ResolvedOptionLabel;
use slug_identity_v2::TargetName;
use slug_identity_v2::serialization::StableSerialize;

#[test]
fn bazel_package_identifier_spellings_canonicalize_main_and_literal_repositories() {
    let main = PackageIdentifier::new(
        CanonicalRepoName::root(),
        PackagePath::parse("pkg/sub").unwrap(),
    );
    for spelling in ["pkg/sub", "//pkg/sub", "@//pkg/sub", "@@//pkg/sub"] {
        assert_eq!(
            PackageIdentifier::parse_bazel_package_identifier(spelling).unwrap(),
            main,
            "{spelling}"
        );
    }

    let apparent_literal =
        PackageIdentifier::parse_bazel_package_identifier("@.literal.repo+.//pkg/sub").unwrap();
    let canonical_literal =
        PackageIdentifier::parse_bazel_package_identifier("@@.literal.repo+.//pkg/sub").unwrap();
    assert_eq!(apparent_literal, canonical_literal);
    let hash = |package: &PackageIdentifier| {
        let mut hasher = DefaultHasher::new();
        package.hash(&mut hasher);
        hasher.finish()
    };
    assert_eq!(hash(&apparent_literal), hash(&canonical_literal));
    assert_eq!(apparent_literal.to_string(), "@@.literal.repo+.//pkg/sub");
    assert_eq!(apparent_literal.repo().as_str(), ".literal.repo+.");
    assert_eq!(apparent_literal.package().as_str(), "pkg/sub");
    for repo in [".repo", "repo.", "..repo", "repo..", "foo+bar"] {
        let parsed =
            PackageIdentifier::parse_bazel_package_identifier(&format!("@{repo}//pkg")).unwrap();
        assert_eq!(parsed.repo().as_str(), repo);
    }

    let root = PackageIdentifier::new(CanonicalRepoName::root(), PackagePath::root());
    for spelling in ["", "//", "@//", "@@//"] {
        assert_eq!(
            PackageIdentifier::parse_bazel_package_identifier(spelling).unwrap(),
            root,
            "{spelling:?}"
        );
    }
}

#[test]
fn bazel_package_identifier_rejects_targets_and_invalid_repository_spellings() {
    for spelling in [
        "pkg:target",
        "//pkg:target",
        "@repo//pkg:target",
        "@@repo//pkg:target",
        "@repo",
        "@@repo",
        "@.//pkg",
        "@..//pkg",
        "@repo~name//pkg",
        "@repo@name//pkg",
        "@repo/name//pkg",
        "@répo//pkg",
        "@repo\u{1}//pkg",
        "@bad!//pkg",
    ] {
        assert!(
            PackageIdentifier::parse_bazel_package_identifier(spelling).is_err(),
            "{spelling}"
        );
    }
}

#[test]
fn bazel_package_identifier_uses_package_specific_ascii_and_component_validation() {
    for package in [
        "pkg",
        "pkg/sub",
        " !\"#$%&'()*+,-.;<=>?@[]^_`{|}~ ",
        "pkg/...suffix",
    ] {
        let parsed = PackageIdentifier::parse_bazel_package_identifier(package).unwrap();
        assert_eq!(parsed.package().as_str(), package);
    }

    for package in [
        "/pkg",
        "pkg/",
        "pkg//sub",
        ".",
        "..",
        "....",
        "pkg/.",
        "pkg/..",
        "pkg/....",
        "pkg/.../sub",
        "pkg/.../...",
        "pkg\\sub",
        "pkg:target",
        "pkg\u{1}",
        "pkg\u{7f}",
        "pkg/é",
    ] {
        assert!(
            PackageIdentifier::parse_bazel_package_identifier(package).is_err(),
            "{package:?}"
        );
    }
}

#[test]
fn bazel_package_identifier_strips_only_a_terminal_exact_triple_dot_component() {
    for (spelling, expected_repo, expected_package) in [
        ("...", "", ""),
        ("pkg/...", "", "pkg"),
        ("@repo//...", "repo", ""),
        ("@@repo//pkg/...", "repo", "pkg"),
    ] {
        let parsed = PackageIdentifier::parse_bazel_package_identifier(spelling).unwrap();
        assert_eq!(parsed.repo().as_str(), expected_repo, "{spelling}");
        assert_eq!(parsed.package().as_str(), expected_package, "{spelling}");
    }
}

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
fn bazel_natural_label_order_is_structural_byte_order_without_mapping_provenance() {
    let shorter_package = CanonicalLabel::parse("@@//a:b/c").unwrap();
    let longer_package = CanonicalLabel::parse("@@//a/b:a").unwrap();
    assert_eq!(
        shorter_package.bazel_natural_cmp(&longer_package),
        Ordering::Less
    );
    assert!(shorter_package.to_string() > longer_package.to_string());

    let bmp = CanonicalLabel::parse("@@//pkg:\u{e000}").unwrap();
    let supplementary = CanonicalLabel::parse("@@//pkg:\u{10000}").unwrap();
    assert_eq!(bmp.bazel_natural_cmp(&supplementary), Ordering::Less);

    let apparent = ApparentLabel::parse("@dep//pkg:target").unwrap();
    let mut first = RepositoryMapping::new(RepositoryMappingId::new("first").unwrap());
    first.insert(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new("canonical").unwrap(),
    );
    let mut second = RepositoryMapping::new(RepositoryMappingId::new("second").unwrap());
    second.insert(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new("canonical").unwrap(),
    );
    let first_label = apparent.resolve(&first);
    let second_label = apparent.resolve(&second);
    assert_ne!(first_label, second_label);
    assert_eq!(
        first_label.bazel_natural_cmp(&second_label),
        Ordering::Equal
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

#[test]
fn option_label_mapping_identity_and_nonvisible_rendering_match_bazel() {
    let mut first = RepositoryMapping::new(RepositoryMappingId::new("first").unwrap());
    first.insert(
        ApparentRepoName::new("alias").unwrap(),
        CanonicalRepoName::new("real+").unwrap(),
    );
    first.insert(
        ApparentRepoName::root(),
        CanonicalRepoName::new("ignored+").unwrap(),
    );
    let mut second = RepositoryMapping::new(RepositoryMappingId::new("second").unwrap());
    second.insert(
        ApparentRepoName::new("alias").unwrap(),
        CanonicalRepoName::new("real+").unwrap(),
    );
    second.insert(
        ApparentRepoName::root(),
        CanonicalRepoName::new("ignored+").unwrap(),
    );
    assert_ne!(first, second);
    assert_eq!(
        first
            .resolve(&ApparentRepoName::new("missing").unwrap())
            .as_str(),
        "missing"
    );
    assert_eq!(first.resolve(&ApparentRepoName::root()).as_str(), "");

    let visible = ResolvedOptionLabel::parse(
        "@alias//p:t",
        OptionLabelContext::MainRepository { mapping: &first },
    )
    .unwrap();
    let same_visible = ResolvedOptionLabel::parse(
        "@alias//p:t",
        OptionLabelContext::MainRepository { mapping: &second },
    )
    .unwrap();
    let direct = ResolvedOptionLabel::parse(
        "@@real+//p:t",
        OptionLabelContext::MainRepository { mapping: &first },
    )
    .unwrap();
    assert_eq!(direct.to_string(), "@@real+//p:t");
    assert_eq!(visible, direct);
    assert_eq!(visible, same_visible);
    assert_ne!(
        ApparentLabel::parse("@alias//p:t").unwrap().resolve(&first),
        ApparentLabel::parse("@alias//p:t")
            .unwrap()
            .resolve(&second)
    );

    let main = RepositoryMapping::new(RepositoryMappingId::new("main").unwrap());
    let package = RepositoryMapping::new(RepositoryMappingId::new("package").unwrap());
    let base = PackageIdentifier::new(
        CanonicalRepoName::new("owner+").unwrap(),
        PackagePath::parse("base").unwrap(),
    );
    let missing_main = ResolvedOptionLabel::parse(
        "@missing//p:t",
        OptionLabelContext::MainRepository { mapping: &main },
    )
    .unwrap();
    let missing_package = ResolvedOptionLabel::parse(
        "@missing//p:t",
        OptionLabelContext::Package {
            base_package: &base,
            mapping: &package,
        },
    )
    .unwrap();
    assert_eq!(
        missing_main.to_string(),
        "@@[unknown repo 'missing' requested from @@]//p:t"
    );
    assert_eq!(
        missing_package.to_string(),
        "@@[unknown repo 'missing' requested from @@owner+]//p:t"
    );
    assert_ne!(missing_main, missing_package);
    assert_ne!(value_hash(&missing_main), value_hash(&missing_package));
    assert_eq!(
        missing_main.bazel_natural_cmp(&missing_package),
        Ordering::Equal
    );

    let direct_missing = ResolvedOptionLabel::parse(
        "@@missing//p:t",
        OptionLabelContext::MainRepository { mapping: &main },
    )
    .unwrap();
    let first_round =
        ResolvedOptionLabel::parse("@missing//p:t", OptionLabelContext::FirstRoundCanonical)
            .unwrap();
    assert_eq!(direct_missing.to_string(), "@@missing//p:t");
    assert_eq!(direct_missing, first_round);
    assert_ne!(direct_missing, missing_main);
    assert_eq!(
        direct_missing.bazel_natural_cmp(&missing_main),
        Ordering::Equal
    );

    let mut ordered = RepositoryMapping::new(RepositoryMappingId::new("tie").unwrap());
    ordered.insert(
        ApparentRepoName::new("baa").unwrap(),
        CanonicalRepoName::new("old+").unwrap(),
    );
    ordered.insert(
        ApparentRepoName::new("aab").unwrap(),
        CanonicalRepoName::new("two+").unwrap(),
    );
    ordered.insert(
        ApparentRepoName::new("baa").unwrap(),
        CanonicalRepoName::new("one+").unwrap(),
    );
    let mut reversed = RepositoryMapping::new(RepositoryMappingId::new("tie").unwrap());
    reversed.insert(
        ApparentRepoName::new("aab").unwrap(),
        CanonicalRepoName::new("two+").unwrap(),
    );
    reversed.insert(
        ApparentRepoName::new("baa").unwrap(),
        CanonicalRepoName::new("one+").unwrap(),
    );
    assert_eq!(ordered, reversed);
    assert_eq!(
        ResolvedOptionLabel::parse(
            "@baa//p:t",
            OptionLabelContext::MainRepository { mapping: &ordered }
        )
        .unwrap()
        .to_string(),
        "@@one+//p:t"
    );
    let ordered_missing = ResolvedOptionLabel::parse(
        "@aaa//p:t",
        OptionLabelContext::MainRepository { mapping: &ordered },
    )
    .unwrap();
    let reversed_missing = ResolvedOptionLabel::parse(
        "@aaa//p:t",
        OptionLabelContext::MainRepository { mapping: &reversed },
    )
    .unwrap();
    assert_eq!(
        ordered_missing.to_string(),
        "@@[unknown repo 'aaa' requested from @@ (did you mean 'baa'?)]//p:t"
    );
    assert_eq!(
        reversed_missing.to_string(),
        "@@[unknown repo 'aaa' requested from @@ (did you mean 'aab'?)]//p:t"
    );
    assert_ne!(ordered_missing, reversed_missing);
    assert_ne!(value_hash(&ordered_missing), value_hash(&reversed_missing));
}

#[test]
fn option_label_context_parses_the_three_bazel_modes() {
    let mut mapping = RepositoryMapping::new(RepositoryMappingId::new("grammar").unwrap());
    mapping.insert(
        ApparentRepoName::root(),
        CanonicalRepoName::new("explicit+").unwrap(),
    );
    mapping.insert(
        ApparentRepoName::new("dep").unwrap(),
        CanonicalRepoName::new("dep+").unwrap(),
    );
    let base = PackageIdentifier::new(
        CanonicalRepoName::new("owner+").unwrap(),
        PackagePath::parse("base/pkg").unwrap(),
    );
    let package = OptionLabelContext::Package {
        base_package: &base,
        mapping: &mapping,
    };
    assert_eq!(
        ResolvedOptionLabel::parse("@//p:t", OptionLabelContext::FirstRoundCanonical)
            .unwrap()
            .to_string(),
        "//p:t"
    );
    for (input, expected) in [
        ("pkg/t:bin", "//pkg/t:bin"),
        (":bin", "//:bin"),
        ("bin", "//bin:bin"),
        ("//conditions:default", "//conditions:default"),
    ] {
        for context in [
            OptionLabelContext::FirstRoundCanonical,
            OptionLabelContext::MainRepository { mapping: &mapping },
        ] {
            assert_eq!(context.parse(input).unwrap().to_string(), expected);
        }
    }
    assert!(package.parse("pkg/t:bin").is_err());
    for input in [":bin", "bin"] {
        assert_eq!(
            package.parse(input).unwrap().to_string(),
            "@@owner+//base/pkg:bin"
        );
    }
    assert_eq!(
        package.parse("//tools:bin").unwrap().to_string(),
        "@@owner+//tools:bin"
    );
    for context in [
        OptionLabelContext::FirstRoundCanonical,
        OptionLabelContext::MainRepository { mapping: &mapping },
    ] {
        assert_eq!(
            context.parse("//p:foo/...").unwrap().to_string(),
            "//p:foo/..."
        );
    }
    assert_eq!(
        package.parse("//p:foo/...").unwrap().to_string(),
        "@@owner+//p:foo/..."
    );
    assert_eq!(
        package.parse(":foo/...").unwrap().to_string(),
        "@@owner+//base/pkg:foo/..."
    );
    assert_eq!(
        ResolvedOptionLabel::parse("@dep//p:foo/...", OptionLabelContext::FirstRoundCanonical)
            .unwrap()
            .to_string(),
        "@@dep//p:foo/..."
    );
    for context in [
        OptionLabelContext::MainRepository { mapping: &mapping },
        package,
    ] {
        assert_eq!(
            context.parse("@dep//p:foo/...").unwrap().to_string(),
            "@@dep+//p:foo/..."
        );
    }
    for special in ["conditions", "visibility"] {
        assert_eq!(
            package
                .parse(&format!("//{special}:default"))
                .unwrap()
                .to_string(),
            format!("//{special}:default")
        );
    }
    assert_eq!(
        package.parse("foo/bar").unwrap().to_string(),
        "@@owner+//base/pkg:foo/bar"
    );

    let explicit_package = package.parse("@//p:t").unwrap();
    let unqualified_package = package.parse("//p:t").unwrap();
    assert_eq!(explicit_package.to_string(), "@@explicit+//p:t");
    assert_eq!(explicit_package.unambiguous_form(), "@@explicit+//p:t");
    assert_ne!(explicit_package, unqualified_package);
    let main_explicit = ResolvedOptionLabel::parse(
        "@//p:t",
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap();
    let main_unqualified = ResolvedOptionLabel::parse(
        "//p:t",
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap();
    assert_eq!(main_explicit.to_string(), "@@explicit+//p:t");
    assert_eq!(main_unqualified.to_string(), "//p:t");
    assert_eq!(main_unqualified.unambiguous_form(), "@@//p:t");

    let mut collapse_main =
        RepositoryMapping::new(RepositoryMappingId::new("collapse-main").unwrap());
    collapse_main.insert(ApparentRepoName::root(), CanonicalRepoName::root());
    let mut collapse_package =
        RepositoryMapping::new(RepositoryMappingId::new("collapse-package").unwrap());
    collapse_package.insert(
        ApparentRepoName::root(),
        CanonicalRepoName::new("owner+").unwrap(),
    );
    let collapsed_main = ResolvedOptionLabel::parse(
        "@//p:t",
        OptionLabelContext::MainRepository {
            mapping: &collapse_main,
        },
    )
    .unwrap();
    let collapsed_package = ResolvedOptionLabel::parse(
        "@//p:t",
        OptionLabelContext::Package {
            base_package: &base,
            mapping: &collapse_package,
        },
    )
    .unwrap();
    assert_eq!(collapsed_main, main_unqualified);
    assert_eq!(collapsed_main.to_string(), "//p:t");
    assert_eq!(collapsed_package, unqualified_package);
    assert_eq!(collapsed_package.to_string(), "@@owner+//p:t");

    let absent = RepositoryMapping::new(RepositoryMappingId::new("absent").unwrap());
    assert_eq!(
        ResolvedOptionLabel::parse(
            "@//p:t",
            OptionLabelContext::MainRepository { mapping: &absent }
        )
        .unwrap()
        .to_string(),
        "@@[unknown repo '' requested from @@]//p:t"
    );
    assert_eq!(
        ResolvedOptionLabel::parse(
            "@//p:t",
            OptionLabelContext::Package {
                base_package: &base,
                mapping: &absent
            }
        )
        .unwrap()
        .to_string(),
        "@@[unknown repo '' requested from @@owner+]//p:t"
    );

    assert_eq!(
        ResolvedOptionLabel::parse("@dep", OptionLabelContext::FirstRoundCanonical)
            .unwrap()
            .to_string(),
        "@@dep//:dep"
    );
    for context in [
        OptionLabelContext::MainRepository { mapping: &mapping },
        package,
    ] {
        assert_eq!(context.parse("@dep").unwrap().to_string(), "@@dep+//:dep");
    }
    for context in [
        OptionLabelContext::FirstRoundCanonical,
        OptionLabelContext::MainRepository { mapping: &mapping },
        package,
    ] {
        assert_eq!(context.parse("@@dep").unwrap().to_string(), "@@dep//:dep");
        assert_eq!(
            context.parse("@@dep//p:t").unwrap().to_string(),
            "@@dep//p:t"
        );
    }
    for spelling in ["/tmp:bin", "//pkg/...", "pkg/...", "@bad!//p:t"] {
        for context in [
            OptionLabelContext::FirstRoundCanonical,
            OptionLabelContext::MainRepository { mapping: &mapping },
            package,
        ] {
            assert!(context.parse(spelling).is_err(), "{spelling}");
        }
    }
}

#[test]
fn package_context_label_parser_covers_complete_bazel_spelling_table() {
    let base = PackageIdentifier::new(
        CanonicalRepoName::new("owner+").unwrap(),
        PackagePath::parse("base/pkg").unwrap(),
    );
    let parse = |input: &str| {
        CanonicalLabel::parse_with_package_context(input, &base, |requested| match requested {
            "alias" => Ok(CanonicalRepoName::new("mapped+").unwrap()),
            "" => Ok(CanonicalRepoName::new("empty+").unwrap()),
            _ => Err(format!("missing apparent repository '{requested}'")),
        })
    };
    for (input, expected) in [
        (":local", "@@owner+//base/pkg:local"),
        ("path/to/target", "@@owner+//base/pkg:path/to/target"),
        ("//tools/compiler", "@@owner+//tools/compiler:compiler"),
        ("//:root", "@@owner+//:root"),
        (
            "@alias//tools/compiler",
            "@@mapped+//tools/compiler:compiler",
        ),
        ("@alias", "@@mapped+//:alias"),
        (
            "@@direct+//tools/compiler",
            "@@direct+//tools/compiler:compiler",
        ),
        ("@@direct+", "@@direct+//:direct+"),
        ("@//tools:empty", "@@empty+//tools:empty"),
        ("@@//tools:main", "@@//tools:main"),
        ("//conditions:default", "@@//conditions:default"),
        ("//visibility:public", "@@//visibility:public"),
        (":dir/name/.", "@@owner+//base/pkg:dir/name"),
    ] {
        assert_eq!(
            parse(input).unwrap(),
            CanonicalLabel::parse(expected).unwrap(),
            "{input}"
        );
    }
    for input in [
        "/single:slash",
        "relative/package:target",
        "//pkg/...:all",
        "@bad!//pkg:target",
    ] {
        assert!(parse(input).is_err(), "{input}");
    }
    assert!(parse("@missing//pkg:target").is_err());
}

#[test]
fn option_label_private_bazel_validators_cover_option_only_spellings() {
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("validator").unwrap());
    let base = PackageIdentifier::new(
        CanonicalRepoName::new("owner+").unwrap(),
        PackagePath::parse("base").unwrap(),
    );
    let package = OptionLabelContext::Package {
        base_package: &base,
        mapping: &mapping,
    };
    for repo in [".foo", "foo.", "repo.."] {
        assert_eq!(
            ResolvedOptionLabel::parse(
                &format!("@{repo}//p:t"),
                OptionLabelContext::FirstRoundCanonical,
            )
            .unwrap()
            .to_string(),
            format!("@@{repo}//p:t")
        );
        for context in [
            OptionLabelContext::MainRepository { mapping: &mapping },
            package,
        ] {
            assert_eq!(
                context
                    .parse(&format!("@@{repo}//p:t"))
                    .unwrap()
                    .to_string(),
                format!("@@{repo}//p:t")
            );
        }
    }
    for input in [
        "@.//p:t",
        "@@..//p:t",
        "@repo~name//p:t",
        "@@repo~name//p:t",
        "//pkg\\name:t",
        "//p\u{e9}:t",
    ] {
        for context in [
            OptionLabelContext::FirstRoundCanonical,
            OptionLabelContext::MainRepository { mapping: &mapping },
            package,
        ] {
            assert!(context.parse(input).is_err(), "{input}");
        }
    }
}

#[test]
fn option_labels_are_structurally_ordered_but_naturally_utf16_compared() {
    let mapping = RepositoryMapping::new(RepositoryMappingId::new("missing").unwrap());
    let visible = ResolvedOptionLabel::parse(
        "@@missing//p:t",
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap();
    let nonvisible = ResolvedOptionLabel::parse(
        "@missing//p:t",
        OptionLabelContext::MainRepository { mapping: &mapping },
    )
    .unwrap();
    assert_ne!(visible, nonvisible);
    assert_ne!(visible.cmp(&nonvisible), Ordering::Equal);
    assert_eq!(visible.bazel_natural_cmp(&nonvisible), Ordering::Equal);
    let supplementary = ResolvedOptionLabel::parse(
        "@@missing//p:\u{10000}",
        OptionLabelContext::FirstRoundCanonical,
    )
    .unwrap();
    let bmp = ResolvedOptionLabel::parse(
        "@@missing//p:\u{e000}",
        OptionLabelContext::FirstRoundCanonical,
    )
    .unwrap();
    assert_eq!(supplementary.bazel_natural_cmp(&bmp), Ordering::Less);
}

fn value_hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
