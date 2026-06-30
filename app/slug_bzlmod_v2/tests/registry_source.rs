use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::parse_registry_metadata_json;
use slug_bzlmod_v2::parse_registry_source_json;

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
