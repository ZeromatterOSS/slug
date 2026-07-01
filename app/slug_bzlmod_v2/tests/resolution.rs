use std::collections::BTreeMap;

use slug_bzlmod_v2::DevDependencyMode;
use slug_bzlmod_v2::ModuleFile;
use slug_bzlmod_v2::ModuleKey;
use slug_bzlmod_v2::ModuleSource;
use slug_bzlmod_v2::bazel_canonical_module_repo_name;
use slug_bzlmod_v2::resolve_local_module_graph;
use slug_bzlmod_v2::resolve_local_module_graph_with_dev_dependency_mode;
use slug_bzlmod_v2::resolve_local_module_graph_with_includes;

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
fn resolves_local_graph_through_included_module_fragment() {
    let root = ModuleFile::parse(
        r#"
module(name = "include_change_root", version = "0.1.0")
include("//:deps.MODULE.bazel")
"#,
    )
    .unwrap();
    let included = ModuleFile::parse(
        r#"
bazel_dep(name = "dep", repo_name = "dep_alias", version = "1.0.0")
local_path_override(module_name = "dep", path = "modules/dep_one")
"#,
    )
    .unwrap();
    let dep = ModuleFile::parse(r#"module(name = "dep", version = "1.0.0")"#).unwrap();

    let graph = resolve_local_module_graph_with_includes(
        &root,
        &BTreeMap::from([("deps.MODULE.bazel".to_owned(), included)]),
        &BTreeMap::from([("dep".to_owned(), dep)]),
    )
    .unwrap();

    assert!(graph.module(&ModuleKey::new("dep", "1.0.0")).is_some());
    assert_eq!(
        graph
            .repo_mapping_for("_main")
            .unwrap()
            .get("dep_alias")
            .map(String::as_str),
        Some("dep+")
    );
    assert_eq!(
        graph
            .module(&ModuleKey::new("dep", "1.0.0"))
            .unwrap()
            .source,
        ModuleSource::LocalPath {
            path: "modules/dep_one".to_owned(),
        }
    );
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
fn local_override_uses_declared_module_version() {
    let root = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0")
bazel_dep(name = "aaa", version = "1.0.0")
local_path_override(module_name = "aaa", path = "modules/aaa")
local_path_override(module_name = "bbb", path = "modules/bbb")
"#,
    )
    .unwrap();
    let aaa = ModuleFile::parse(
        r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#,
    )
    .unwrap();
    let bbb = ModuleFile::parse(r#"module(name = "bbb", version = "2.0.0")"#).unwrap();
    let locals = BTreeMap::from([("aaa".to_owned(), aaa), ("bbb".to_owned(), bbb)]);

    let graph = resolve_local_module_graph(&root, &locals).unwrap();
    let bbb_key = ModuleKey::new("bbb", "2.0.0");
    assert!(graph.module(&bbb_key).is_some());
    let aaa_mapping = graph.repo_mapping_for("aaa+").unwrap();
    assert_eq!(aaa_mapping.get("bbb").map(String::as_str), Some("bbb+"));
}

#[test]
fn local_override_version_selection_is_request_order_independent() {
    let root = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0")
bazel_dep(name = "ccc", version = "1.0.0")
bazel_dep(name = "aaa", version = "1.0.0")
local_path_override(module_name = "aaa", path = "modules/aaa")
local_path_override(module_name = "ccc", path = "modules/ccc")
local_path_override(module_name = "bbb", path = "modules/bbb")
"#,
    )
    .unwrap();
    let ccc = ModuleFile::parse(
        r#"
module(name = "ccc", version = "1.0.0")
bazel_dep(name = "bbb", version = "2.0.0")
"#,
    )
    .unwrap();
    let aaa = ModuleFile::parse(
        r#"
module(name = "aaa", version = "1.0.0")
bazel_dep(name = "bbb", version = "1.0.0")
"#,
    )
    .unwrap();
    let bbb = ModuleFile::parse(r#"module(name = "bbb", version = "2.0.0")"#).unwrap();
    let locals = BTreeMap::from([
        ("aaa".to_owned(), aaa),
        ("bbb".to_owned(), bbb),
        ("ccc".to_owned(), ccc),
    ]);

    let graph = resolve_local_module_graph(&root, &locals).unwrap();
    let bbb_key = ModuleKey::new("bbb", "2.0.0");
    assert!(graph.module(&bbb_key).is_some());
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

#[test]
fn local_graph_root_dev_dependencies_follow_mode() {
    let root = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0")
bazel_dep(name = "dep", version = "1.0.0", dev_dependency = True)
local_path_override(module_name = "dep", path = "modules/dep")
"#,
    )
    .unwrap();
    let dep = ModuleFile::parse(r#"module(name = "dep", version = "1.0.0")"#).unwrap();
    let locals = BTreeMap::from([("dep".to_owned(), dep)]);

    let default_graph = resolve_local_module_graph(&root, &locals).unwrap();
    assert!(
        default_graph
            .module(&ModuleKey::new("dep", "1.0.0"))
            .is_some()
    );
    assert_eq!(
        default_graph
            .repo_mapping_for("_main")
            .unwrap()
            .get("dep")
            .map(String::as_str),
        Some("dep+")
    );

    let ignored_graph = resolve_local_module_graph_with_dev_dependency_mode(
        &root,
        &locals,
        DevDependencyMode::IgnoreRoot,
    )
    .unwrap();
    assert!(
        ignored_graph
            .module(&ModuleKey::new("dep", "1.0.0"))
            .is_none()
    );
    assert!(
        !ignored_graph
            .repo_mapping_for("_main")
            .unwrap()
            .contains_key("dep")
    );
}

#[test]
fn local_graph_ignores_non_root_dev_dependencies() {
    let root = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0")
bazel_dep(name = "parent", version = "1.0.0")
local_path_override(module_name = "parent", path = "modules/parent")
local_path_override(module_name = "child", path = "modules/child")
"#,
    )
    .unwrap();
    let parent = ModuleFile::parse(
        r#"
module(name = "parent", version = "1.0.0")
bazel_dep(name = "child", version = "1.0.0", dev_dependency = True)
"#,
    )
    .unwrap();
    let child = ModuleFile::parse(r#"module(name = "child", version = "1.0.0")"#).unwrap();
    let locals = BTreeMap::from([("parent".to_owned(), parent), ("child".to_owned(), child)]);

    let graph = resolve_local_module_graph(&root, &locals).unwrap();

    assert!(graph.module(&ModuleKey::new("parent", "1.0.0")).is_some());
    assert!(graph.module(&ModuleKey::new("child", "1.0.0")).is_none());
    assert!(
        !graph
            .repo_mapping_for("parent+")
            .unwrap()
            .contains_key("child")
    );
}

#[test]
fn local_graph_rejects_root_overrides_for_nonexistent_modules() {
    let root = ModuleFile::parse(
        r#"
module(name = "root", version = "0.0.0")
bazel_dep(name = "dep", version = "1.0.0")
local_path_override(module_name = "dep", path = "modules/dep")
local_path_override(module_name = "local_missing", path = "missing")
single_version_override(module_name = "single_missing", version = "1.0.0")
multiple_version_override(module_name = "multi_missing", versions = ["1.0.0", "2.0.0"])
archive_override(module_name = "archive_missing", urls = ["file:///archive.zip"])
git_override(module_name = "git_missing", remote = "https://example.invalid/repo.git", commit = "0123456789012345678901234567890123456789")
"#,
    )
    .unwrap();
    let dep = ModuleFile::parse(r#"module(name = "dep", version = "1.0.0")"#).unwrap();
    let locals = BTreeMap::from([("dep".to_owned(), dep)]);

    let err = resolve_local_module_graph(&root, &locals).unwrap_err();
    assert!(err.contains(
        "root module specifies overrides on nonexistent module(s): local_missing, single_missing, multi_missing, archive_missing, git_missing"
    ));
}
