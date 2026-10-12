use std::sync::Arc;

use compact_str::CompactString;
use slug_identity_v2::CanonicalLabel;
use starlark_map::small_map::SmallMap;

use super::*;

type Attributes = SmallMap<CompactString, OverrideAttributeValue>;
const NAME: &str = "rust_linux_x86_64__x86_64-unknown-linux-gnu__nightly_tools";
const LABEL: &str = "@@rules_rust+//rust/private:repositories.bzl";
const EXPORT: &str = "rust_toolchain_tools_repository";

fn attributes() -> Attributes {
    let mut attributes = Attributes::from_iter(
        [
            ("version", "nightly/2025-09-14"),
            ("exec_triple", "x86_64-unknown-linux-gnu"),
            ("target_triple", "x86_64-unknown-linux-gnu"),
            ("edition", "2024"),
            ("rustfmt_version", "nightly/2026-07-16"),
        ]
        .map(|(key, value)| {
            (
                CompactString::new(key),
                OverrideAttributeValue::String(value.into()),
            )
        }),
    );
    attributes.insert("dev_components".into(), OverrideAttributeValue::Bool(false));
    attributes.insert(
        "urls".into(),
        OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::String(
            "https://static.rust-lang.org/dist/{}.tar.xz".into(),
        )])),
    );
    attributes.insert(
        "sha256s".into(),
        OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter(DECLARED_NIGHTLY_HASHES.map(
            |(key, value)| {
                (
                    OverrideAttributeKey::String(key.into()),
                    OverrideAttributeValue::String(value.into()),
                )
            },
        )))),
    );
    attributes
}

fn declaration() -> Vec<RepositoryRuleAttribute> {
    [
        ("netrc", AttributeKind::String),
        ("auth_patterns", AttributeKind::StringList),
        ("auth", AttributeKind::StringDict),
    ]
    .map(|(name, kind)| RepositoryRuleAttribute {
        name: name.into(),
        kind,
        mandatory: false,
        default: None,
        file_admissibility: crate::attrs::FileAdmissibility::default(),
    })
    .into()
}

#[test]
fn nightly_profile_requires_exact_selected_inputs() {
    let attributes = attributes();
    let declaration = declaration();
    let label = CanonicalLabel::parse(LABEL).unwrap();
    let linux = RepositoryPlatform::new("linux", "x86_64");
    let selected = |attributes: &Attributes| {
        select_parts(NAME, &label, EXPORT, attributes, &declaration, &linux)
    };
    assert_eq!(
        selected(&attributes),
        Some(SelectedArchiveProfile::LinuxNightly)
    );
    for name in attributes.keys() {
        let missing = Attributes::from_iter(
            attributes
                .iter()
                .filter(|(key, _)| *key != name)
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        assert_eq!(selected(&missing), None, "missing {name}");
        for wrong in [
            OverrideAttributeValue::None,
            OverrideAttributeValue::Int(1),
            OverrideAttributeValue::String("wrong".into()),
        ] {
            let mut changed = attributes.clone();
            changed.insert(name.clone(), wrong);
            assert_eq!(selected(&changed), None, "wrong {name}");
        }
    }
    for (name, value) in [
        ("dev_components", OverrideAttributeValue::Bool(true)),
        (
            "urls",
            OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::String(
                "https://mirror.invalid/{}.tar.xz".into(),
            )])),
        ),
        (
            "urls",
            OverrideAttributeValue::Iterable(Arc::from([
                OverrideAttributeValue::String(
                    "https://static.rust-lang.org/dist/{}.tar.xz".into(),
                ),
                OverrideAttributeValue::String("https://mirror.invalid/{}.tar.xz".into()),
            ])),
        ),
    ] {
        let mut changed = attributes.clone();
        changed.insert(name.into(), value);
        assert_eq!(selected(&changed), None);
    }
    for (name, defining, export, os, arch) in [
        ("other", LABEL, EXPORT, "linux", "x86_64"),
        (
            NAME,
            "@@other+//rust/private:repositories.bzl",
            EXPORT,
            "linux",
            "x86_64",
        ),
        (
            NAME,
            "@@rules_rust+//other:repositories.bzl",
            EXPORT,
            "linux",
            "x86_64",
        ),
        (
            NAME,
            "@@rules_rust+//rust/private:other.bzl",
            EXPORT,
            "linux",
            "x86_64",
        ),
        (NAME, LABEL, "other", "linux", "x86_64"),
        (NAME, LABEL, EXPORT, "mac os x", "x86_64"),
        (NAME, LABEL, EXPORT, "linux", "aarch64"),
    ] {
        assert_eq!(
            select_parts(
                name,
                &CanonicalLabel::parse(defining).unwrap(),
                export,
                &attributes,
                &declaration,
                &RepositoryPlatform::new(os, arch)
            ),
            None
        );
    }
    assert_eq!(
        select_parts(
            "rust_host_tools",
            &label,
            EXPORT,
            &Attributes::new(),
            &[],
            &RepositoryPlatform::new("mac os x", "aarch64")
        ),
        Some(SelectedArchiveProfile::Stable)
    );
    for (key, _) in DECLARED_NIGHTLY_HASHES {
        let mut changed = attributes.clone();
        let OverrideAttributeValue::Map(hashes) = changed.get_mut("sha256s").unwrap() else {
            unreachable!()
        };
        Arc::make_mut(hashes).insert(
            OverrideAttributeKey::String(key.into()),
            OverrideAttributeValue::String("0".repeat(64).into()),
        );
        assert_eq!(selected(&changed), None, "changed {key}");
    }
    let mut changed = attributes.clone();
    let OverrideAttributeValue::Map(hashes) = changed.get_mut("sha256s").unwrap() else {
        unreachable!()
    };
    Arc::make_mut(hashes).insert(
        OverrideAttributeKey::String("extra".into()),
        OverrideAttributeValue::String("0".repeat(64).into()),
    );
    assert_eq!(selected(&changed), None);
    for (name, empty, nonempty) in [
        (
            "netrc",
            OverrideAttributeValue::String("".into()),
            OverrideAttributeValue::String("credentials".into()),
        ),
        (
            "auth_patterns",
            OverrideAttributeValue::Iterable(Arc::from([])),
            OverrideAttributeValue::Iterable(Arc::from([OverrideAttributeValue::String(
                "*".into(),
            )])),
        ),
        (
            "auth",
            OverrideAttributeValue::Map(Arc::new(SmallMap::new())),
            OverrideAttributeValue::Map(Arc::new(SmallMap::from_iter([(
                OverrideAttributeKey::String("url".into()),
                OverrideAttributeValue::String("secret".into()),
            )]))),
        ),
    ] {
        for valid in [OverrideAttributeValue::None, empty] {
            let mut changed = attributes.clone();
            changed.insert(name.into(), valid);
            assert_eq!(
                selected(&changed),
                Some(SelectedArchiveProfile::LinuxNightly)
            );
        }
        for invalid in [nonempty, OverrideAttributeValue::Int(1)] {
            let mut changed = attributes.clone();
            changed.insert(name.into(), invalid);
            assert_eq!(selected(&changed), None);
        }
    }
    for index in 0..declaration.len() {
        let mut changed = declaration.clone();
        changed[index].kind = AttributeKind::Integer;
        assert_eq!(
            select_parts(NAME, &label, EXPORT, &attributes, &changed, &linux),
            None
        );
        let mut changed = declaration.clone();
        changed[index].default = Some(CoercedAttributeValue::String("nonempty".into()));
        assert_eq!(
            select_parts(NAME, &label, EXPORT, &attributes, &changed, &linux),
            None
        );
    }
}

#[test]
fn nightly_archive_guard_requires_exact_six_ordered_tuples() {
    let digest = |hex: &str| {
        std::array::from_fn(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap())
    };
    let build = |order: &[usize], change: Option<usize>, file_at: usize| {
        let mut builder = GeneratedRepositoryFileEffectPlan::builder();
        for (ordinal, index) in order.iter().enumerate() {
            if ordinal == file_at {
                builder
                    .push("generated".into(), Arc::from(b"content".as_slice()), false)
                    .unwrap();
            }
            let (url, hash, prefix) = NIGHTLY_ARCHIVES[*index];
            let mut urls = vec![CompactString::new(if change == Some(0) && ordinal == 0 {
                "https://static.rust-lang.org/dist/other.tar.xz"
            } else {
                url
            })];
            if change == Some(3) && ordinal == 0 {
                urls.push(CompactString::new(url));
            }
            builder
                .push_archive(
                    urls.into(),
                    if change == Some(1) && ordinal == 0 {
                        [0; 32]
                    } else {
                        digest(hash)
                    },
                    if change == Some(2) && ordinal == 0 {
                        "other/component".into()
                    } else {
                        prefix.into()
                    },
                )
                .unwrap();
        }
        if file_at == order.len() {
            builder
                .push("generated".into(), Arc::from(b"content".as_slice()), false)
                .unwrap();
        }
        builder.finish()
    };
    let profile = SelectedArchiveProfile::LinuxNightly;
    let all = [0, 1, 2, 3, 4, 5];
    assert!(profile.matches_plan(&build(&all, None, 6)));
    assert!(!SelectedArchiveProfile::Stable.matches_plan(&build(&all, None, 6)));
    for order in [&all[..5], &[1, 0, 2, 3, 4, 5], &[0, 1, 2, 3, 4, 4]] {
        assert!(!profile.matches_plan(&build(order, None, usize::MAX)));
    }
    for change in 0..4 {
        assert!(!profile.matches_plan(&build(&all, Some(change), 6)));
    }
    for file_at in [0, 3] {
        assert!(!profile.matches_plan(&build(&all, None, file_at)));
    }
    let mut extra = GeneratedRepositoryFileEffectPlan::builder();
    for (url, hash, prefix) in NIGHTLY_ARCHIVES {
        extra
            .push_archive(
                Arc::from([CompactString::new(url)]),
                digest(hash),
                prefix.into(),
            )
            .unwrap();
    }
    // A seventh distinct archive is refused by the existing plan owner's hard cap.
    assert!(matches!(
        extra.push_archive(
            Arc::from([CompactString::new(
                "https://static.rust-lang.org/dist/other.tar.xz"
            )]),
            digest(NIGHTLY_ARCHIVES[0].1),
            "extra/component".into(),
        ),
        Err(slug_bzlmod_v2::GeneratedRepositoryFileEffectPlanError::TooManyArchives)
    ));
}
