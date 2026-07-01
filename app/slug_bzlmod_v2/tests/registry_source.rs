use std::collections::BTreeMap;

use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::RegistrySource;
use slug_bzlmod_v2::RegistrySourceCatalog;
use slug_bzlmod_v2::digest_module_file_content;
use slug_bzlmod_v2::digest_selected_registry_sources;
use slug_bzlmod_v2::parse_registry_metadata_json;
use slug_bzlmod_v2::parse_registry_source_json;
use slug_bzlmod_v2::registry_source_json_url;
use slug_bzlmod_v2::selected_registry_file_hash_urls;

fn key() -> ModuleKey {
    ModuleKey::new("srcmod", "1.0.0")
}

#[test]
fn parses_archive_source_json_metadata() {
    let source = r#"{
        "url": "file:///archive.tar.gz",
        "integrity": "sha256-archive",
        "strip_prefix": "srcmod-1.0.0",
        "patches": {"fix.patch": "sha256-patch"},
        "patch_strip": 1
    }"#;

    let parsed = parse_registry_source_json(&key(), source).unwrap();

    assert_eq!(parsed.urls, ["file:///archive.tar.gz"]);
    assert_eq!(parsed.integrity, "sha256-archive");
    assert_eq!(parsed.strip_prefix.as_deref(), Some("srcmod-1.0.0"));
    assert_eq!(
        parsed.patches.get("fix.patch").map(String::as_str),
        Some("sha256-patch")
    );
    assert_eq!(parsed.patch_strip, Some(1));
}

#[test]
fn accepts_urls_list_form() {
    let parsed = parse_registry_source_json(
        &key(),
        r#"{"urls":["file:///one.tar.gz","https://example.invalid/two.tar.gz"],"integrity":"sha256-list"}"#,
    )
    .unwrap();

    assert_eq!(
        parsed.urls,
        ["file:///one.tar.gz", "https://example.invalid/two.tar.gz"]
    );
    assert_eq!(parsed.integrity, "sha256-list");
}

#[test]
fn rejects_missing_source_url_like_bazel() {
    let err = parse_registry_source_json(&key(), r#"{"integrity":"sha256-archive"}"#).unwrap_err();
    assert!(err.contains("Missing source URL for module srcmod@1.0.0"));
}

#[test]
fn rejects_missing_integrity_like_bazel() {
    let err =
        parse_registry_source_json(&key(), r#"{"url":"file:///archive.tar.gz"}"#).unwrap_err();
    assert!(err.contains("Missing integrity for module srcmod@1.0.0"));
}

#[test]
fn rejects_invalid_json_with_bazel_shaped_prefix() {
    let err = parse_registry_source_json(&key(), "{not json}\n").unwrap_err();
    assert!(err.contains("Unable to parse json at url"));
}

#[test]
fn parses_registry_metadata_yanked_versions() {
    let metadata = parse_registry_metadata_json(
        "yyy",
        r#"{
            "homepage": "https://example.invalid/yyy",
            "repository": ["https://example.invalid/yyy.git"],
            "versions": ["1.0.0", "2.0.0"],
            "yanked_versions": {"1.0.0": "bad release"}
        }"#,
    )
    .unwrap();

    assert_eq!(
        metadata.homepage.as_deref(),
        Some("https://example.invalid/yyy")
    );
    assert_eq!(metadata.repository, ["https://example.invalid/yyy.git"]);
    assert_eq!(metadata.versions, ["1.0.0", "2.0.0"]);
    assert_eq!(
        metadata
            .yanked_version_entries("yyy")
            .get(&ModuleKey::new("yyy", "1.0.0"))
            .map(String::as_str),
        Some("bad release")
    );
}

#[test]
fn rejects_registry_metadata_without_versions() {
    let err =
        parse_registry_metadata_json("yyy", r#"{"yanked_versions":{"1.0.0":"bad"}}"#).unwrap_err();
    assert!(err.contains("metadata.json for module yyy is missing versions"));
}

#[test]
fn rejects_invalid_registry_metadata_json() {
    let err = parse_registry_metadata_json("yyy", "{not json}\n").unwrap_err();
    assert!(err.contains("Unable to parse json at url metadata.json"));
}

#[test]
fn ordered_registry_source_selection_preserves_source_json_digests() {
    let first_json = r#"{"url":"file:///first.tar.gz","integrity":"sha256-first"}"#;
    let second_json = r#"{"url":"file:///second.tar.gz","integrity":"sha256-second"}"#;
    let first = RegistrySourceCatalog::with_source_json_digests(
        "file:///%workspace%/first",
        BTreeMap::from([(
            key(),
            (
                parse_registry_source_json(&key(), first_json).unwrap(),
                digest_module_file_content(first_json),
            ),
        )]),
    )
    .unwrap();
    let second = RegistrySourceCatalog::with_source_json_digests(
        "file:///%workspace%/second",
        BTreeMap::from([(
            key(),
            (
                parse_registry_source_json(&key(), second_json).unwrap(),
                digest_module_file_content(second_json),
            ),
        )]),
    )
    .unwrap();

    let selected = slug_bzlmod_v2::select_ordered_registry_sources(&[first, second]);

    let selected_source = selected.get(&key()).unwrap();
    assert_eq!(selected_source.registry_url, "file:///%workspace%/first");
    assert_eq!(selected_source.spec.integrity, "sha256-first");
    assert_eq!(
        selected_source.source_json_digest(),
        Some(digest_module_file_content(first_json).as_str())
    );

    let digest = digest_selected_registry_sources(&selected).unwrap();
    let changed = RegistrySourceCatalog::with_source_json_digests(
        "file:///%workspace%/first",
        BTreeMap::from([(
            key(),
            (
                parse_registry_source_json(&key(), second_json).unwrap(),
                digest_module_file_content(second_json),
            ),
        )]),
    )
    .unwrap();
    let changed_digest =
        digest_selected_registry_sources(&slug_bzlmod_v2::select_ordered_registry_sources(&[
            changed,
        ]))
        .unwrap();

    assert_ne!(digest, changed_digest);
}

#[test]
fn selected_registry_source_digest_requires_explicit_digests() {
    let source = RegistrySource::new(
        "file:///%workspace%/registry",
        parse_registry_source_json(
            &key(),
            r#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
        )
        .unwrap(),
    );

    let err = digest_selected_registry_sources(&BTreeMap::from([(key(), source)])).unwrap_err();
    assert!(err.contains("selected registry source srcmod@1.0.0 has no source.json digest"));
}
#[test]
fn registry_source_hash_urls_match_bazel_lockfile_shape() {
    assert_eq!(
        registry_source_json_url(
            "https://bcr.bazel.build/",
            &ModuleKey::new("rules_cc", "0.2.17")
        ),
        "https://bcr.bazel.build/modules/rules_cc/0.2.17/source.json"
    );
}

#[test]
fn selected_registry_file_hash_urls_include_modules_and_sources() {
    let module_file =
        slug_bzlmod_v2::ModuleFile::parse(r#"module(name = "srcmod", version = "1.0.0")"#).unwrap();
    let modules = BTreeMap::from([(
        key(),
        slug_bzlmod_v2::RegistryModule::new("https://bcr.bazel.build/", module_file),
    )]);
    let source = RegistrySource::new(
        "https://bcr.bazel.build/",
        parse_registry_source_json(
            &key(),
            r#"{"url":"file:///archive.tar.gz","integrity":"sha256-archive"}"#,
        )
        .unwrap(),
    );

    assert_eq!(
        selected_registry_file_hash_urls(&modules, &BTreeMap::from([(key(), source)])),
        [
            "https://bcr.bazel.build/modules/srcmod/1.0.0/MODULE.bazel".to_owned(),
            "https://bcr.bazel.build/modules/srcmod/1.0.0/source.json".to_owned(),
        ]
    );
}
