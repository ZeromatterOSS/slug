use std::collections::BTreeMap;

use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::digest_module_file_content;
use slug_bzlmod_v2::snapshot_registry_contents;

fn key() -> ModuleKey {
    ModuleKey::new("srcmod", "1.0.0")
}

#[test]
fn snapshot_registry_contents_parses_and_hashes_fetched_files() {
    let registry_json = r#"{"mirrors":[],"module_base_path":"modules"}"#;
    let module_file = r#"module(name = "srcmod", version = "1.0.0")"#;
    let source_json = r#"{"url":"file:///srcmod.tar.gz","integrity":"sha256-archive"}"#;

    let snapshot = snapshot_registry_contents(
        "https://bcr.bazel.build/",
        registry_json,
        BTreeMap::from([(key(), module_file.to_owned())]),
        BTreeMap::from([(key(), source_json.to_owned())]),
    )
    .unwrap();

    assert_eq!(
        snapshot.registry_policy_entry.url(),
        "https://bcr.bazel.build/"
    );
    assert_eq!(
        snapshot.registry_policy_entry.digest(),
        digest_module_file_content(registry_json)
    );
    assert_eq!(snapshot.module_catalog.modules.len(), 1);
    assert_eq!(snapshot.source_catalog.sources.len(), 1);

    let observed = snapshot.observed_file_hashes().unwrap();
    assert_eq!(
        observed.get("https://bcr.bazel.build/bazel_registry.json"),
        Some(&digest_module_file_content(registry_json))
    );
    assert_eq!(
        observed.get("https://bcr.bazel.build/modules/srcmod/1.0.0/MODULE.bazel"),
        Some(&digest_module_file_content(module_file))
    );
    assert_eq!(
        observed.get("https://bcr.bazel.build/modules/srcmod/1.0.0/source.json"),
        Some(&digest_module_file_content(source_json))
    );
}

#[test]
fn snapshot_registry_contents_produces_aggregate_dice_digests() {
    let registry_json = r#"{"mirrors":[],"module_base_path":"modules"}"#;
    let module_file = r#"module(name = "srcmod", version = "1.0.0")"#;
    let source_json = r#"{"url":"file:///srcmod.tar.gz","integrity":"sha256-archive"}"#;

    let snapshot = snapshot_registry_contents(
        "https://bcr.bazel.build/",
        registry_json,
        BTreeMap::from([(key(), module_file.to_owned())]),
        BTreeMap::from([(key(), source_json.to_owned())]),
    )
    .unwrap();
    let digests = snapshot.dice_input_digests().unwrap();

    assert_eq!(
        digests.registry_policy_digest,
        slug_bzlmod_v2::digest_registry_policy([snapshot.registry_policy_entry.clone()])
    );
    assert_ne!(
        digests.registry_module_digest,
        digests.registry_source_digest
    );

    let changed_source = snapshot_registry_contents(
        "https://bcr.bazel.build/",
        registry_json,
        BTreeMap::from([(key(), module_file.to_owned())]),
        BTreeMap::from([(
            key(),
            r#"{"url":"file:///srcmod.tar.gz","integrity":"sha256-other"}"#.to_owned(),
        )]),
    )
    .unwrap()
    .dice_input_digests()
    .unwrap();

    assert_eq!(
        digests.registry_policy_digest,
        changed_source.registry_policy_digest
    );
    assert_eq!(
        digests.registry_module_digest,
        changed_source.registry_module_digest
    );
    assert_ne!(
        digests.registry_source_digest,
        changed_source.registry_source_digest
    );
}

#[test]
fn snapshot_registry_contents_rejects_mismatched_module_path_and_header() {
    let err = snapshot_registry_contents(
        "https://bcr.bazel.build/",
        r#"{}"#,
        BTreeMap::from([(
            key(),
            r#"module(name = "other", version = "1.0.0")"#.to_owned(),
        )]),
        BTreeMap::new(),
    )
    .unwrap_err();

    assert!(err.contains("registry MODULE.bazel for module srcmod@1.0.0 declares other@1.0.0"));
}

#[test]
fn snapshot_registry_contents_rejects_invalid_registry_index_json() {
    let err = snapshot_registry_contents(
        "https://bcr.bazel.build/",
        "not json",
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .unwrap_err();

    assert!(
        err.contains("Unable to parse bazel_registry.json for registry https://bcr.bazel.build/")
    );
}
