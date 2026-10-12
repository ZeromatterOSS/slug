use slug_bzlmod_v2::GeneratedRepositoryFileEffectPlan;
use slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef;
use slug_bzlmod_v2::OverrideAttributeKey;
use slug_bzlmod_v2::OverrideAttributeValue;
use slug_bzlmod_v2::RepositoryPlatform;

use super::HostInstantiatedModuleExtensionRepository;
use crate::attrs::AttributeKind;
use crate::attrs::CoercedAttributeValue;
use crate::module_extension_repository_rule::RepositoryRuleAttribute;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectedArchiveProfile {
    Stable,
    LinuxNightly,
}

// Pinned rules_rust 0.73.0 repositories.bzl's real ordered tool calls.
// rustfmt retains the original default nightly date, separately from the root version.
const NIGHTLY_ARCHIVES: [(&str, &str, &str); 6] = [
    (
        "https://static.rust-lang.org/dist/2025-09-14/rustc-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "0aa143cf401827874af7114f6466c0c87d4e31dfd6a2355c50db2e0c71bd45c7",
        "rustc-nightly-x86_64-unknown-linux-gnu/rustc",
    ),
    (
        "https://static.rust-lang.org/dist/2025-09-14/clippy-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "98bb5375ef6648d6e59fe8558e12df854e73653db2b6c7ee8891ffa0336f7622",
        "clippy-nightly-x86_64-unknown-linux-gnu/clippy-preview",
    ),
    (
        "https://static.rust-lang.org/dist/2025-09-14/cargo-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "cbc76e93946fb4658b692f9d71dbd4861f942b5b2bf6e34b072ba708d00e89e8",
        "cargo-nightly-x86_64-unknown-linux-gnu/cargo",
    ),
    (
        "https://static.rust-lang.org/dist/2026-07-16/rustfmt-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "bb48a8124f84818dab540701952e4cf5506045000baa975b6046d54aeac87d5d",
        "rustfmt-nightly-x86_64-unknown-linux-gnu/rustfmt-preview",
    ),
    (
        "https://static.rust-lang.org/dist/2025-09-14/llvm-tools-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "7302ec9a0447d5e21d918428b629a1d76ff934a562660e19b275fad707cf83dc",
        "llvm-tools-nightly-x86_64-unknown-linux-gnu/llvm-tools-preview",
    ),
    (
        "https://static.rust-lang.org/dist/2025-09-14/rust-std-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "9a0a34cc7a11eb74f408ab7a5eafd063bcce3726c9bb89351ed7f2811fa35b1a",
        "rust-std-nightly-x86_64-unknown-linux-gnu/rust-std-x86_64-unknown-linux-gnu",
    ),
];
const DECLARED_NIGHTLY_HASHES: [(&str, &str); 6] = [
    (
        "2025-09-14/rustc-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "0aa143cf401827874af7114f6466c0c87d4e31dfd6a2355c50db2e0c71bd45c7",
    ),
    (
        "2025-09-14/clippy-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "98bb5375ef6648d6e59fe8558e12df854e73653db2b6c7ee8891ffa0336f7622",
    ),
    (
        "2025-09-14/cargo-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "cbc76e93946fb4658b692f9d71dbd4861f942b5b2bf6e34b072ba708d00e89e8",
    ),
    (
        "2025-09-14/rustfmt-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "d9206e55ee07ae3ba9de7190142a8e422a72f300726fa8e094a6300c7128aa82",
    ),
    (
        "2025-09-14/llvm-tools-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "7302ec9a0447d5e21d918428b629a1d76ff934a562660e19b275fad707cf83dc",
    ),
    (
        "2025-09-14/rust-std-nightly-x86_64-unknown-linux-gnu.tar.xz",
        "9a0a34cc7a11eb74f408ab7a5eafd063bcce3726c9bb89351ed7f2811fa35b1a",
    ),
];

impl SelectedArchiveProfile {
    pub(super) fn select(
        repository: &HostInstantiatedModuleExtensionRepository,
        platform: &RepositoryPlatform,
    ) -> Option<Self> {
        let definition = &repository.call().definition;
        select_parts(
            repository.generated_name(),
            &definition.defining_label,
            &definition.exported_name,
            &repository.spec_parts().1.attributes,
            &definition.attributes,
            platform,
        )
    }

    pub(super) fn matches_plan(self, plan: &GeneratedRepositoryFileEffectPlan) -> bool {
        let expected = match self {
            Self::Stable => &super::SELECTED_HOST_TOOLS_ARCHIVES,
            Self::LinuxNightly => &NIGHTLY_ARCHIVES,
        };
        let mut effects = plan.ordered_effects();
        for (url, sha256, strip_prefix) in expected {
            let Some(GeneratedRepositoryOrderedEffectRef::Archive(archive)) = effects.next() else {
                return false;
            };
            if archive.urls().len() != 1
                || archive.urls()[0].as_str() != *url
                || !sha256_matches(archive.sha256(), sha256)
                || archive.strip_prefix() != *strip_prefix
            {
                return false;
            }
        }
        effects.all(|effect| matches!(effect, GeneratedRepositoryOrderedEffectRef::File(_)))
    }
}

fn select_parts(
    generated_name: &str,
    defining_label: &slug_identity_v2::CanonicalLabel,
    exported_name: &str,
    attributes: &starlark_map::small_map::SmallMap<
        compact_str::CompactString,
        OverrideAttributeValue,
    >,
    declaration: &[RepositoryRuleAttribute],
    platform: &RepositoryPlatform,
) -> Option<SelectedArchiveProfile> {
    // Preserve the original stable selector exactly, without new attribute restrictions.
    if generated_name == "rust_host_tools"
        && defining_label.to_string() == "@@rules_rust+//rust/private:repositories.bzl"
        && exported_name == "rust_toolchain_tools_repository"
    {
        return Some(SelectedArchiveProfile::Stable);
    }
    let label = defining_label;
    if generated_name != "rust_linux_x86_64__x86_64-unknown-linux-gnu__nightly_tools"
        || label.package().repo().as_str() != "rules_rust+"
        || label.package().package().as_str() != "rust/private"
        || label.target().as_str() != "repositories.bzl"
        || exported_name != "rust_toolchain_tools_repository"
        || platform.os_name() != "linux"
        || platform.arch() != "x86_64"
    {
        return None;
    }
    if nightly_attributes_match(attributes) && authentication_is_empty(attributes, declaration) {
        Some(SelectedArchiveProfile::LinuxNightly)
    } else {
        None
    }
}

fn nightly_attributes_match(
    attributes: &starlark_map::small_map::SmallMap<
        compact_str::CompactString,
        OverrideAttributeValue,
    >,
) -> bool {
    for (name, expected) in [
        ("version", "nightly/2025-09-14"),
        ("exec_triple", "x86_64-unknown-linux-gnu"),
        ("target_triple", "x86_64-unknown-linux-gnu"),
        ("edition", "2024"),
        ("rustfmt_version", "nightly/2026-07-16"),
    ] {
        if !matches!(attributes.get(name), Some(OverrideAttributeValue::String(value)) if value == expected)
        {
            return false;
        }
    }
    if !matches!(
        attributes.get("dev_components"),
        Some(OverrideAttributeValue::Bool(false))
    ) {
        return false;
    }
    let Some(OverrideAttributeValue::Iterable(urls)) = attributes.get("urls") else {
        return false;
    };
    if !matches!(urls.as_ref(), [OverrideAttributeValue::String(url)] if url == "https://static.rust-lang.org/dist/{}.tar.xz")
    {
        return false;
    }
    let Some(OverrideAttributeValue::Map(hashes)) = attributes.get("sha256s") else {
        return false;
    };
    if hashes.len() != DECLARED_NIGHTLY_HASHES.len()
        || !DECLARED_NIGHTLY_HASHES.iter().all(|(key, expected)| {
            hashes.iter().any(|(name, value)| {
                matches!(
                    (name, value),
                    (OverrideAttributeKey::String(name), OverrideAttributeValue::String(value))
                        if name.as_str() == *key && value.as_str() == *expected
                )
            })
        })
    {
        return false;
    }
    true
}

fn authentication_is_empty(
    attributes: &starlark_map::small_map::SmallMap<
        compact_str::CompactString,
        OverrideAttributeValue,
    >,
    declaration: &[RepositoryRuleAttribute],
) -> bool {
    [
        ("netrc", AttributeKind::String),
        ("auth_patterns", AttributeKind::StringList),
        ("auth", AttributeKind::StringDict),
    ]
    .into_iter()
    .all(|(name, kind)| {
        let Some(schema) = declaration.iter().find(|attribute| attribute.name == name) else {
            return false;
        };
        if schema.kind != kind {
            return false;
        }
        match attributes.get(name) {
            None | Some(OverrideAttributeValue::None) => match (kind, &schema.default) {
                (_, None) => true,
                (AttributeKind::String, Some(CoercedAttributeValue::String(value))) => {
                    value.is_empty()
                }
                (AttributeKind::StringList, Some(CoercedAttributeValue::StringList(value))) => {
                    value.is_empty()
                }
                (AttributeKind::StringDict, Some(CoercedAttributeValue::StringDict(value))) => {
                    value.is_empty()
                }
                _ => false,
            },
            Some(OverrideAttributeValue::String(value)) if kind == AttributeKind::String => {
                value.is_empty()
            }
            Some(OverrideAttributeValue::Iterable(value)) if kind == AttributeKind::StringList => {
                value.is_empty()
            }
            Some(OverrideAttributeValue::Map(value)) if kind == AttributeKind::StringDict => {
                value.is_empty()
            }
            _ => false,
        }
    })
}

fn sha256_matches(actual: [u8; 32], expected: &str) -> bool {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    expected.len() == 64
        && actual
            .iter()
            .zip(expected.as_bytes().chunks_exact(2))
            .all(|(byte, pair)| {
                pair[0] == HEX[(byte >> 4) as usize] && pair[1] == HEX[(byte & 15) as usize]
            })
}

#[cfg(test)]
#[path = "selected_archive_profile_tests.rs"]
mod tests;
