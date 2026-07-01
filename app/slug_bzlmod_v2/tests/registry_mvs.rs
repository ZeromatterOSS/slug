use std::collections::BTreeMap;
use std::collections::BTreeSet;

use slug_bzlmod_v2::BzlmodRegistryPolicyEntry;
use slug_bzlmod_v2::ModuleFile;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::ModuleSource;
use slug_bzlmod_v2::RegistryCatalog;
use slug_bzlmod_v2::RegistryModule;
use slug_bzlmod_v2::YankedVersionPolicy;
use slug_bzlmod_v2::digest_module_file_content;
use slug_bzlmod_v2::digest_selected_registry_modules;
use slug_bzlmod_v2::observed_registry_file_hashes;
use slug_bzlmod_v2::observed_registry_policy_file_hashes;
use slug_bzlmod_v2::registry_bazel_registry_json_url;
use slug_bzlmod_v2::registry_module_file_url;
use slug_bzlmod_v2::resolve_registry_mvs;
use slug_bzlmod_v2::select_ordered_registry_modules;
use slug_bzlmod_v2::validate_yanked_versions;

fn module(source: &str) -> ModuleFile {
    ModuleFile::parse(source).unwrap()
}

fn registry_module(source: &str) -> RegistryModule {
    RegistryModule::new("file:///%workspace%/registry", module(source))
}

#[test]
fn registry_mvs_selects_highest_requested_transitive_version() {
    let root = module(
        r#"
module(name = "registry_mvs_root", version = "0.1.0")
bazel_dep(name = "aaa", version = "1.0.0")
bazel_dep(name = "ccc", version = "1.0.0")
"#,
    );
    let registry_modules = BTreeMap::from([
        (
            ModuleKey::new("aaa", "1.0.0"),
            registry_module(
                r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("ccc", "1.0.0"),
            registry_module(
                r#"
module(name = "ccc", version = "1.0.0")
bazel_dep(name = "bbb", version = "2.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("bbb", "1.0.0"),
            registry_module(r#"module(name = "bbb", version = "1.0.0")"#),
        ),
        (
            ModuleKey::new("bbb", "2.0.0"),
            registry_module(r#"module(name = "bbb", version = "2.0.0")"#),
        ),
    ]);

    let graph = resolve_registry_mvs(&root, &registry_modules).unwrap();
    let bbb_1 = ModuleKey::new("bbb", "1.0.0");
    let bbb_2 = ModuleKey::new("bbb", "2.0.0");

    assert!(graph.module(&bbb_1).is_none());
    assert!(graph.module(&bbb_2).is_some());
    assert_eq!(
        graph.module(&bbb_2).unwrap().source,
        ModuleSource::Registry {
            registry_url: "file:///%workspace%/registry".to_owned(),
        }
    );
    assert_eq!(
        graph
            .repo_mapping_for("aaa+")
            .unwrap()
            .get("bbb")
            .map(String::as_str),
        Some("bbb+")
    );
    assert_eq!(
        graph
            .repo_mapping_for("ccc+")
            .unwrap()
            .get("bbb")
            .map(String::as_str),
        Some("bbb+")
    );
}

#[test]
fn registry_mvs_does_not_downgrade_after_higher_version_selected() {
    let root = module(
        r#"
module(name = "root", version = "0.1.0")
bazel_dep(name = "ccc", version = "1.0.0")
bazel_dep(name = "aaa", version = "1.0.0")
"#,
    );
    let registry_modules = BTreeMap::from([
        (
            ModuleKey::new("ccc", "1.0.0"),
            registry_module(
                r#"
module(name = "ccc", version = "1.0.0")
bazel_dep(name = "bbb", version = "2.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("aaa", "1.0.0"),
            registry_module(
                r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("bbb", "1.0.0"),
            registry_module(r#"module(name = "bbb", version = "1.0.0")"#),
        ),
        (
            ModuleKey::new("bbb", "2.0.0"),
            registry_module(r#"module(name = "bbb", version = "2.0.0")"#),
        ),
    ]);

    let graph = resolve_registry_mvs(&root, &registry_modules).unwrap();
    assert!(graph.module(&ModuleKey::new("bbb", "1.0.0")).is_none());
    assert!(graph.module(&ModuleKey::new("bbb", "2.0.0")).is_some());
}

#[test]
fn registry_mvs_reports_missing_requested_module_version() {
    let root = module(
        r#"
module(name = "root", version = "0.1.0")
bazel_dep(name = "aaa", version = "1.0.0")
"#,
    );

    let err = resolve_registry_mvs(&root, &BTreeMap::new()).unwrap_err();
    assert!(err.contains("registry module aaa@1.0.0 was not supplied"));
}

#[test]
fn yanked_versions_are_rejected_unless_allowed() {
    let root = module(
        r#"
module(name = "root", version = "0.1.0")
bazel_dep(name = "yyy", version = "1.0.0")
"#,
    );
    let registry_modules = BTreeMap::from([(
        ModuleKey::new("yyy", "1.0.0"),
        registry_module(r#"module(name = "yyy", version = "1.0.0")"#),
    )]);
    let graph = resolve_registry_mvs(&root, &registry_modules).unwrap();
    let yanked = BTreeMap::from([(ModuleKey::new("yyy", "1.0.0"), "bad release".to_owned())]);

    let err = validate_yanked_versions(&graph, &yanked, &YankedVersionPolicy::Reject).unwrap_err();
    assert!(err.contains("Yanked version detected in your resolved dependency graph: yyy@1.0.0"));
    assert!(err.contains("bad release"));

    let allowed = validate_yanked_versions(
        &graph,
        &yanked,
        &YankedVersionPolicy::AllowList(BTreeSet::from([ModuleKey::new("yyy", "1.0.0")])),
    )
    .unwrap();
    assert_eq!(allowed.len(), 1);
    assert_eq!(allowed[0].module, ModuleKey::new("yyy", "1.0.0"));

    assert!(validate_yanked_versions(&graph, &yanked, &YankedVersionPolicy::AllowAll).is_ok());
}

#[test]
fn multiple_version_override_keeps_requested_versions_and_repo_mappings() {
    let root = module(
        r#"
module(name = "root", version = "0.1.0")
bazel_dep(name = "aaa", version = "1.0.0")
bazel_dep(name = "ccc", version = "1.0.0")
multiple_version_override(module_name = "bbb", versions = ["1.0.0", "2.0.0"])
"#,
    );
    let registry_modules = BTreeMap::from([
        (
            ModuleKey::new("aaa", "1.0.0"),
            registry_module(
                r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("ccc", "1.0.0"),
            registry_module(
                r#"
module(name = "ccc", version = "1.0.0")
bazel_dep(name = "bbb", version = "2.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("bbb", "1.0.0"),
            registry_module(r#"module(name = "bbb", version = "1.0.0")"#),
        ),
        (
            ModuleKey::new("bbb", "2.0.0"),
            registry_module(r#"module(name = "bbb", version = "2.0.0")"#),
        ),
    ]);

    let graph = resolve_registry_mvs(&root, &registry_modules).unwrap();

    assert_eq!(
        graph
            .module(&ModuleKey::new("bbb", "1.0.0"))
            .map(|module| module.canonical_repo.as_str()),
        Some("bbb+1.0.0")
    );
    assert_eq!(
        graph
            .module(&ModuleKey::new("bbb", "2.0.0"))
            .map(|module| module.canonical_repo.as_str()),
        Some("bbb+2.0.0")
    );
    assert_eq!(
        graph
            .repo_mapping_for("aaa+")
            .unwrap()
            .get("bbb")
            .map(String::as_str),
        Some("bbb+1.0.0")
    );
    assert_eq!(
        graph
            .repo_mapping_for("ccc+")
            .unwrap()
            .get("bbb")
            .map(String::as_str),
        Some("bbb+2.0.0")
    );
}

#[test]
fn bazel_repo_mappings_match_canonical_names_oracle_shape() {
    let root = module(
        r#"
module(name = "repo_mapping_root", version = "0.1.0")
bazel_dep(name = "aaa", version = "1.0.0")
bazel_dep(name = "ccc", version = "1.0.0")
multiple_version_override(module_name = "bbb", versions = ["1.0.0", "2.0.0"])
"#,
    );
    let registry_modules = BTreeMap::from([
        (
            ModuleKey::new("aaa", "1.0.0"),
            registry_module(
                r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("ccc", "1.0.0"),
            registry_module(
                r#"
module(name = "ccc", version = "1.0.0")
bazel_dep(name = "bbb", version = "2.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("bbb", "1.0.0"),
            registry_module(r#"module(name = "bbb", version = "1.0.0")"#),
        ),
        (
            ModuleKey::new("bbb", "2.0.0"),
            registry_module(r#"module(name = "bbb", version = "2.0.0")"#),
        ),
    ]);

    let graph = resolve_registry_mvs(&root, &registry_modules).unwrap();

    assert_eq!(
        graph.bazel_repo_mapping_for("aaa+").unwrap(),
        BTreeMap::from([
            ("aaa".to_owned(), "aaa+".to_owned()),
            ("bbb".to_owned(), "bbb+1.0.0".to_owned()),
            ("bazel_tools".to_owned(), "bazel_tools".to_owned()),
        ])
    );
    assert_eq!(
        graph.bazel_repo_mapping_for("ccc+").unwrap(),
        BTreeMap::from([
            ("ccc".to_owned(), "ccc+".to_owned()),
            ("bbb".to_owned(), "bbb+2.0.0".to_owned()),
            ("bazel_tools".to_owned(), "bazel_tools".to_owned()),
        ])
    );
    assert_eq!(
        graph.bazel_repo_mapping_for("bbb+1.0.0").unwrap(),
        BTreeMap::from([
            ("bbb".to_owned(), "bbb+1.0.0".to_owned()),
            ("bazel_tools".to_owned(), "bazel_tools".to_owned()),
        ])
    );

    assert_eq!(
        graph
            .extension_generated_repo_mapping("+ext+generated", "generated")
            .unwrap(),
        BTreeMap::from([
            ("".to_owned(), "".to_owned()),
            ("repo_mapping_root".to_owned(), "".to_owned()),
            ("generated".to_owned(), "+ext+generated".to_owned()),
            ("aaa".to_owned(), "aaa+".to_owned()),
            ("ccc".to_owned(), "ccc+".to_owned()),
            ("bazel_tools".to_owned(), "bazel_tools".to_owned()),
        ])
    );
}
#[test]
fn multiple_version_override_rejects_unlisted_requested_version() {
    let root = module(
        r#"
module(name = "root", version = "0.1.0")
bazel_dep(name = "bbb", version = "3.0.0")
multiple_version_override(module_name = "bbb", versions = ["1.0.0", "2.0.0"])
"#,
    );
    let registry_modules = BTreeMap::from([(
        ModuleKey::new("bbb", "3.0.0"),
        registry_module(r#"module(name = "bbb", version = "3.0.0")"#),
    )]);

    let err = resolve_registry_mvs(&root, &registry_modules).unwrap_err();
    assert!(err.contains(
        "multiple_version_override for module bbb does not allow requested version 3.0.0"
    ));
}

#[test]
fn single_version_override_replaces_requested_version() {
    let root = module(
        r#"
module(name = "root", version = "0.1.0")
bazel_dep(name = "aaa", version = "1.0.0")
single_version_override(module_name = "bbb", version = "2.0.0")
"#,
    );
    let registry_modules = BTreeMap::from([
        (
            ModuleKey::new("aaa", "1.0.0"),
            registry_module(
                r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#,
            ),
        ),
        (
            ModuleKey::new("bbb", "1.0.0"),
            registry_module(r#"module(name = "bbb", version = "1.0.0")"#),
        ),
        (
            ModuleKey::new("bbb", "2.0.0"),
            registry_module(r#"module(name = "bbb", version = "2.0.0")"#),
        ),
    ]);

    let graph = resolve_registry_mvs(&root, &registry_modules).unwrap();

    assert!(graph.module(&ModuleKey::new("bbb", "1.0.0")).is_none());
    assert_eq!(
        graph
            .module(&ModuleKey::new("bbb", "2.0.0"))
            .map(|module| module.canonical_repo.as_str()),
        Some("bbb+")
    );
    assert_eq!(
        graph
            .repo_mapping_for("aaa+")
            .unwrap()
            .get("bbb")
            .map(String::as_str),
        Some("bbb+")
    );
}

#[test]
fn ordered_registry_selection_uses_first_hit_and_later_misses() {
    let first = RegistryCatalog::new(
        "file:///%workspace%/first",
        BTreeMap::from([(
            ModuleKey::new("aaa", "1.0.0"),
            module(r#"module(name = "aaa", version = "1.0.0")"#),
        )]),
    );
    let second = RegistryCatalog::new(
        "file:///%workspace%/second",
        BTreeMap::from([
            (
                ModuleKey::new("aaa", "1.0.0"),
                module(
                    r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#,
                ),
            ),
            (
                ModuleKey::new("ccc", "1.0.0"),
                module(r#"module(name = "ccc", version = "1.0.0")"#),
            ),
        ]),
    );

    let selected = select_ordered_registry_modules(&[first, second]);

    assert_eq!(
        selected
            .get(&ModuleKey::new("aaa", "1.0.0"))
            .map(|module| module.registry_url.as_str()),
        Some("file:///%workspace%/first")
    );
    assert_eq!(
        selected
            .get(&ModuleKey::new("ccc", "1.0.0"))
            .map(|module| module.registry_url.as_str()),
        Some("file:///%workspace%/second")
    );
    assert!(
        selected
            .get(&ModuleKey::new("aaa", "1.0.0"))
            .unwrap()
            .module_file
            .directives
            .iter()
            .all(|directive| !matches!(directive, slug_bzlmod_v2::Directive::BazelDep(_)))
    );
}

#[test]
fn ordered_registry_selection_preserves_module_file_digests() {
    let first_aaa = r#"module(name = "aaa", version = "1.0.0")"#;
    let second_aaa = r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#;
    let second_ccc = r#"module(name = "ccc", version = "1.0.0")"#;
    let first = RegistryCatalog::with_module_file_digests(
        "file:///%workspace%/first",
        BTreeMap::from([(
            ModuleKey::new("aaa", "1.0.0"),
            (module(first_aaa), digest_module_file_content(first_aaa)),
        )]),
    )
    .unwrap();
    let second = RegistryCatalog::with_module_file_digests(
        "file:///%workspace%/second",
        BTreeMap::from([
            (
                ModuleKey::new("aaa", "1.0.0"),
                (module(second_aaa), digest_module_file_content(second_aaa)),
            ),
            (
                ModuleKey::new("ccc", "1.0.0"),
                (module(second_ccc), digest_module_file_content(second_ccc)),
            ),
        ]),
    )
    .unwrap();

    let selected = select_ordered_registry_modules(&[first, second]);

    assert_eq!(
        selected
            .get(&ModuleKey::new("aaa", "1.0.0"))
            .and_then(RegistryModule::module_file_digest),
        Some(digest_module_file_content(first_aaa).as_str())
    );
    assert_eq!(
        selected
            .get(&ModuleKey::new("ccc", "1.0.0"))
            .and_then(RegistryModule::module_file_digest),
        Some(digest_module_file_content(second_ccc).as_str())
    );

    let digest = digest_selected_registry_modules(&selected).unwrap();
    let first_changed = RegistryCatalog::with_module_file_digests(
        "file:///%workspace%/first",
        BTreeMap::from([(
            ModuleKey::new("aaa", "1.0.0"),
            (module(second_aaa), digest_module_file_content(second_aaa)),
        )]),
    )
    .unwrap();
    let changed =
        digest_selected_registry_modules(&select_ordered_registry_modules(&[first_changed]))
            .unwrap();

    assert_ne!(digest, changed);
}

#[test]
fn selected_registry_module_digest_requires_explicit_digests() {
    let selected = select_ordered_registry_modules(&[RegistryCatalog::new(
        "file:///%workspace%/registry",
        BTreeMap::from([(
            ModuleKey::new("aaa", "1.0.0"),
            module(r#"module(name = "aaa", version = "1.0.0")"#),
        )]),
    )]);

    let err = digest_selected_registry_modules(&selected).unwrap_err();
    assert!(err.contains("selected registry module aaa@1.0.0 has no module file digest"));
}

#[test]
fn yanked_policy_parses_environment_allowlist() {
    assert_eq!(
        YankedVersionPolicy::from_env_value(None).unwrap(),
        YankedVersionPolicy::Reject
    );
    assert_eq!(
        YankedVersionPolicy::from_env_value(Some("all")).unwrap(),
        YankedVersionPolicy::AllowAll
    );
    assert_eq!(
        YankedVersionPolicy::from_env_value(Some("yyy@1.0.0, zzz@2.0.0")).unwrap(),
        YankedVersionPolicy::AllowList(BTreeSet::from([
            ModuleKey::new("yyy", "1.0.0"),
            ModuleKey::new("zzz", "2.0.0"),
        ]))
    );
}

#[test]
fn yanked_policy_rejects_invalid_environment_entries() {
    let err = YankedVersionPolicy::from_env_value(Some("yyy")).unwrap_err();

    assert!(err.contains("BZLMOD_ALLOW_YANKED_VERSIONS entry yyy must be 'all' or module@version"));
}
#[test]
fn registry_module_hash_urls_match_bazel_lockfile_shape() {
    let module = ModuleKey::new("rules_cc", "0.2.17");

    assert_eq!(
        registry_module_file_url("https://bcr.bazel.build/", &module),
        "https://bcr.bazel.build/modules/rules_cc/0.2.17/MODULE.bazel"
    );
    assert_eq!(
        registry_bazel_registry_json_url("https://bcr.bazel.build/"),
        "https://bcr.bazel.build/bazel_registry.json"
    );
}

#[test]
fn observed_registry_hashes_include_selected_module_digests() {
    let module_source = r#"module(name = "aaa", version = "1.0.0")"#;
    let selected = select_ordered_registry_modules(&[RegistryCatalog::with_module_file_digests(
        "https://bcr.bazel.build/",
        BTreeMap::from([(
            ModuleKey::new("aaa", "1.0.0"),
            (
                module(module_source),
                digest_module_file_content(module_source),
            ),
        )]),
    )
    .unwrap()]);

    let hashes = observed_registry_file_hashes(&selected, &BTreeMap::new()).unwrap();

    assert_eq!(
        hashes.get("https://bcr.bazel.build/modules/aaa/1.0.0/MODULE.bazel"),
        Some(&digest_module_file_content(module_source))
    );
}

#[test]
fn observed_registry_hashes_require_selected_module_digests() {
    let selected = select_ordered_registry_modules(&[RegistryCatalog::new(
        "https://bcr.bazel.build/",
        BTreeMap::from([(
            ModuleKey::new("aaa", "1.0.0"),
            module(r#"module(name = "aaa", version = "1.0.0")"#),
        )]),
    )]);

    let err = observed_registry_file_hashes(&selected, &BTreeMap::new()).unwrap_err();
    assert!(err.contains("selected registry module aaa@1.0.0 has no MODULE.bazel digest"));
}

#[test]
fn observed_registry_policy_hashes_include_bazel_registry_json_digest() {
    let digest = digest_module_file_content(b"bazel registry metadata");
    let entry = BzlmodRegistryPolicyEntry::new("https://bcr.bazel.build/", digest.clone()).unwrap();

    let hashes = observed_registry_policy_file_hashes([&entry]).unwrap();

    assert_eq!(
        hashes.get("https://bcr.bazel.build/bazel_registry.json"),
        Some(&digest)
    );
}

#[test]
fn observed_registry_policy_hashes_reject_conflicting_registry_digests() {
    let first = BzlmodRegistryPolicyEntry::new(
        "https://bcr.bazel.build/",
        digest_module_file_content(b"first registry metadata"),
    )
    .unwrap();
    let second = BzlmodRegistryPolicyEntry::new(
        "https://bcr.bazel.build",
        digest_module_file_content(b"second registry metadata"),
    )
    .unwrap();

    let err = observed_registry_policy_file_hashes([&first, &second]).unwrap_err();

    assert!(err.contains(
        "conflicting observed registry file hash for https://bcr.bazel.build/bazel_registry.json"
    ));
}
