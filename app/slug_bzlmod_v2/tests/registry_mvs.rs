use std::collections::BTreeMap;

use slug_bzlmod_v2::ModuleFile;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::ModuleSource;
use slug_bzlmod_v2::RegistryModule;
use slug_bzlmod_v2::resolve_registry_mvs;

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
