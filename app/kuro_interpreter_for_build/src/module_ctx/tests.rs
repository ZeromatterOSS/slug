/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::path::PathBuf;

use tempfile::TempDir;

use crate::module_ctx::context::ModuleContext;
use crate::module_ctx::context::SerializedModule;
use crate::module_ctx::module::BazelModule;
use crate::module_ctx::os::RepositoryOs;
use crate::module_ctx::tags::SerializedTag;
use crate::module_ctx::tags::SerializedTagValue;

#[test]
fn test_module_context_empty() {
    let ctx = ModuleContext::empty();
    assert!(ctx.get_modules().is_empty());
    assert!(!ctx.has_working_dir());
    assert!(ctx.working_dir().is_none());
    // delete_on_close is always true for module_ctx
    assert!(ctx.should_delete_working_dir());
}

#[test]
fn test_module_context_exposes_facts_attr() {
    use starlark::environment::Module;
    use starlark::values::StarlarkValue;

    let module = Module::new();
    let heap = module.heap();
    let ctx = ModuleContext::empty().with_facts(serde_json::json!({"resource": "stored"}));

    assert!(ctx.has_attr("facts", heap));
    let facts = ctx.get_attr("facts", heap).unwrap();
    assert!(facts.is_in(heap.alloc("resource")).unwrap());
    assert_eq!(
        facts.at(heap.alloc("resource"), heap).unwrap().unpack_str(),
        Some("stored")
    );
}

#[test]
fn test_module_context_extension_metadata_returns_facts() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;
    use starlark::values::ValueLike;

    use crate::module_ctx::StarlarkModuleExtensionMetadata;

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ModuleContext::empty()));

    let ast = AstModule::parse(
        "metadata.star",
        "mctx.extension_metadata(facts = {'resource': {'checksum': 'abc'}})".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let result = eval.eval_module(ast, &Globals::standard()).unwrap();
    let metadata = result
        .downcast_ref::<StarlarkModuleExtensionMetadata>()
        .unwrap();

    assert_eq!(
        metadata.metadata().facts,
        serde_json::json!({"resource": {"checksum": "abc"}})
    );
}

#[test]
fn test_module_context_with_temp_working_dir() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let ctx = ModuleContext::empty().with_temp_working_dir(temp_path.clone());

    assert!(ctx.has_working_dir());
    assert_eq!(ctx.working_dir().unwrap(), temp_path.as_path());
    // delete_on_close is always true for module_ctx
    assert!(ctx.should_delete_working_dir());
}

#[test]
fn test_module_context_resolve_path_relative() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let ctx = ModuleContext::empty().with_temp_working_dir(temp_path.clone());

    let resolved = ctx.resolve_path("subdir/file.txt").unwrap();
    assert_eq!(resolved, temp_path.join("subdir/file.txt"));
}

#[test]
fn test_module_context_resolve_path_absolute() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let ctx = ModuleContext::empty().with_temp_working_dir(temp_path);

    let absolute = "/absolute/path/to/file.txt";
    let resolved = ctx.resolve_path(absolute).unwrap();
    assert_eq!(resolved, PathBuf::from(absolute));
}

#[test]
fn test_module_context_resolve_path_no_working_dir() {
    let ctx = ModuleContext::empty();
    assert!(ctx.resolve_path("some/file.txt").is_none());
}

#[test]
fn test_module_context_new_has_no_working_dir() {
    let modules = vec![BazelModule::new(
        "test_module".to_owned(),
        "1.0.0".to_owned(),
        true,
        vec!["install".to_owned()],
    )];
    let ctx = ModuleContext::new(modules, true);

    // New contexts don't have working dir by default
    assert!(!ctx.has_working_dir());
    assert!(ctx.working_dir().is_none());
    // But delete_on_close is still true
    assert!(ctx.should_delete_working_dir());
}

#[test]
fn test_module_context_from_serialized_has_no_working_dir() {
    let modules = vec![SerializedModule {
        name: "test_module".to_owned(),
        version: "1.0.0".to_owned(),
        is_root: true,
        tags_by_class: HashMap::new(),
    }];
    let ctx = ModuleContext::from_serialized(modules, false);

    // New contexts don't have working dir by default
    assert!(!ctx.has_working_dir());
    assert!(ctx.working_dir().is_none());
    // But delete_on_close is still true
    assert!(ctx.should_delete_working_dir());
}

#[test]
fn test_module_context_working_dir_is_temporary() {
    // This test verifies the key difference from repository_ctx:
    // module_ctx working dir should always be marked for deletion
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    let ctx = ModuleContext::empty().with_temp_working_dir(temp_path);

    // Key difference: module_ctx always deletes working dir
    assert!(ctx.should_delete_working_dir());
}

#[test]
fn test_bazel_module_creation() {
    let module = BazelModule::new(
        "rules_python".to_owned(),
        "0.31.0".to_owned(),
        false,
        vec!["install".to_owned(), "pip".to_owned()],
    );

    assert_eq!(module.name(), "rules_python");
    assert_eq!(module.version(), "0.31.0");
    assert!(!module.is_root());
    assert!(module.tags_by_class().contains_key("install"));
    assert!(module.tags_by_class().contains_key("pip"));
}

#[test]
fn test_bazel_module_with_tags() {
    let mut tags_by_class = HashMap::new();
    tags_by_class.insert(
        "install".to_owned(),
        vec![SerializedTag::new(vec![
            (
                "name".to_owned(),
                SerializedTagValue::String("numpy".to_owned()),
            ),
            (
                "version".to_owned(),
                SerializedTagValue::String("1.24.0".to_owned()),
            ),
        ])],
    );

    let module = BazelModule::with_tags(
        "rules_python".to_owned(),
        "0.31.0".to_owned(),
        true,
        tags_by_class.clone(),
    );

    assert_eq!(module.name(), "rules_python");
    assert!(module.is_root());
    assert_eq!(module.tags_by_class().len(), 1);
    assert!(module.tags_by_class().get("install").unwrap().len() == 1);
}

#[test]
fn test_repository_os() {
    let os = RepositoryOs::new();

    // Just verify it creates something - actual values depend on platform
    assert!(!os.name.is_empty());
    assert!(!os.arch.is_empty());
}
