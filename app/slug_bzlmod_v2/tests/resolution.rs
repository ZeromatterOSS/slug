use std::collections::BTreeMap;

use slug_bzlmod_v2::ModuleFile;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::ModuleSource;
use slug_bzlmod_v2::bazel_canonical_module_repo_name;
use slug_bzlmod_v2::resolve_local_module_graph;

#[test]
fn resolves_transitive_local_path_overrides() {
    let root = ModuleFile::parse(
        r#"
module(name = "module_resolution_basic", version = "0.0.0")
bazel_dep(name = "aaa", version = "1.0.0")
local_path_override(module_name = "aaa", path = "modules/aaa")
local_path_override(module_name = "bbb", path = "modules/bbb")
"#,
    )
    .unwrap();
    let aaa = ModuleFile::parse(
        r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "2.0.0")
"#,
    )
    .unwrap();
    let bbb = ModuleFile::parse(r#"module(name = "bbb", version = "2.0.0")"#).unwrap();

    let mut locals = BTreeMap::new();
    locals.insert("aaa".to_owned(), aaa);
    locals.insert("bbb".to_owned(), bbb);

    let graph = resolve_local_module_graph(&root, &locals).unwrap();
    let root_key = ModuleKey::new("module_resolution_basic", "0.0.0");
    let aaa_key = ModuleKey::new("aaa", "1.0.0");
    let bbb_key = ModuleKey::new("bbb", "2.0.0");

    assert_eq!(graph.root, root_key);
    assert_eq!(graph.modules.len(), 3);
    assert_eq!(
        graph.module(&aaa_key).unwrap().source,
        ModuleSource::LocalPath {
            path: "modules/aaa".to_owned(),
        }
    );
    assert_eq!(
        graph.module(&bbb_key).unwrap().source,
        ModuleSource::LocalPath {
            path: "modules/bbb".to_owned(),
        }
    );
    assert_eq!(graph.module(&aaa_key).unwrap().canonical_repo, "aaa+");
    assert_eq!(graph.module(&bbb_key).unwrap().canonical_repo, "bbb+");

    let root_mapping = graph.repo_mapping_for("_main").unwrap();
    assert_eq!(root_mapping.get("aaa").map(String::as_str), Some("aaa+"));
    let aaa_mapping = graph.repo_mapping_for("aaa+").unwrap();
    assert_eq!(aaa_mapping.get("bbb").map(String::as_str), Some("bbb+"));
}

#[test]
fn repo_name_controls_apparent_root_mapping_name() {
    let root = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0")
bazel_dep(name = "dep", version = "1.0.0", repo_name = "dep_alias")
local_path_override(module_name = "dep", path = "modules/dep")
"#,
    )
    .unwrap();
    let dep = ModuleFile::parse(r#"module(name = "dep", version = "1.0.0")"#).unwrap();
    let locals = BTreeMap::from([("dep".to_owned(), dep)]);

    let graph = resolve_local_module_graph(&root, &locals).unwrap();
    let root_mapping = graph.repo_mapping_for("_main").unwrap();
    assert_eq!(
        root_mapping.get("dep_alias").map(String::as_str),
        Some("dep+")
    );
    assert!(!root_mapping.contains_key("dep"));
}

#[test]
fn reports_missing_local_override() {
    let root = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0")
bazel_dep(name = "dep", version = "1.0.0")
"#,
    )
    .unwrap();

    let err = resolve_local_module_graph(&root, &BTreeMap::new()).unwrap_err();
    assert!(err.contains("module dep has no local_path_override"));
}

#[test]
fn reports_local_module_name_mismatch() {
    let root = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0")
bazel_dep(name = "dep", version = "1.0.0")
local_path_override(module_name = "dep", path = "modules/dep")
"#,
    )
    .unwrap();
    let wrong = ModuleFile::parse(r#"module(name = "other", version = "1.0.0")"#).unwrap();
    let locals = BTreeMap::from([("dep".to_owned(), wrong)]);

    let err = resolve_local_module_graph(&root, &locals).unwrap_err();
    assert!(err.contains("local module dep declared module name other"));
}

#[test]
fn canonical_module_repo_name_matches_bazel_local_graph_shape() {
    assert_eq!(bazel_canonical_module_repo_name("aaa"), "aaa+");
    assert_eq!(bazel_canonical_module_repo_name("_main"), "_main");
}
