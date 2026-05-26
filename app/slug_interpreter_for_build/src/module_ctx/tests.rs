/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

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
fn test_module_context_repo_env_is_context_owned() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let mut repo_env = BTreeMap::new();
    repo_env.insert("PLAN61_REPO_ENV".to_owned(), "from-context".to_owned());
    let ctx = ModuleContext::empty().with_repo_env(Arc::new(repo_env));

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "repo_env.star",
        "mctx.getenv('PLAN61_REPO_ENV') + ':' + mctx.os.environ['PLAN61_REPO_ENV']".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let result = eval.eval_module(ast, &Globals::standard()).unwrap();

    assert_eq!(result.unpack_str(), Some("from-context:from-context"));
}

#[test]
fn test_module_context_records_getenv_inputs() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let mut repo_env = BTreeMap::new();
    repo_env.insert("PLAN61_REPO_ENV".to_owned(), "from-context".to_owned());
    let ctx = ModuleContext::empty().with_repo_env(Arc::new(repo_env));
    let ctx_handle = ctx.clone();

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "repo_env.star",
        "mctx.getenv('PLAN61_REPO_ENV') + ':' + mctx.getenv('PLAN61_REPO_ENV') + ':' + str(mctx.getenv('PLAN61_MISSING'))".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let result = eval.eval_module(ast, &Globals::standard()).unwrap();

    assert_eq!(result.unpack_str(), Some("from-context:from-context:None"));
    assert_eq!(
        ctx_handle.recorded_inputs().unwrap(),
        vec![
            "ENV:PLAN61_REPO_ENV from-context".to_owned(),
            "ENV:PLAN61_MISSING \\0".to_owned(),
        ]
    );
}

#[test]
fn test_module_context_records_watch_file_inputs() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let temp_dir = TempDir::new().unwrap();
    let watched = temp_dir.path().join("watched.txt");
    let working_dir = temp_dir.path().join("work");
    std::fs::write(&watched, "first\n").unwrap();
    std::fs::create_dir_all(&working_dir).unwrap();
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir)
        .with_label_resolution(temp_dir.path().to_path_buf(), HashMap::new());
    let ctx_handle = ctx.clone();

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "watch.star",
        format!("mctx.watch({:?})", watched.to_string_lossy()),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let result = eval.eval_module(ast, &Globals::standard()).unwrap();

    assert!(result.is_none());
    assert_eq!(
        ctx_handle.recorded_inputs().unwrap(),
        vec![
            slug_bzlmod::recorded_file_input_with_recorded_path(
                PathBuf::from("@@//watched.txt").as_path(),
                &watched,
            )
            .unwrap()
        ]
    );
}

#[test]
fn test_module_context_rejects_watch_under_working_dir() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let temp_dir = TempDir::new().unwrap();
    let watched = temp_dir.path().join("watched.txt");
    std::fs::write(&watched, "first\n").unwrap();
    let ctx = ModuleContext::empty().with_temp_working_dir(temp_dir.path().to_path_buf());

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "watch.star",
        "mctx.watch('watched.txt')".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let err = eval.eval_module(ast, &Globals::standard()).unwrap_err();

    assert!(
        err.to_string()
            .contains("attempted to watch path under working directory"),
        "{err:?}"
    );
}

#[test]
fn test_module_context_rejects_conflicting_recorded_input_values() {
    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().join("work");
    std::fs::create_dir_all(&working_dir).unwrap();
    let watched = temp_dir.path().join("watched.txt");
    std::fs::write(&watched, "first\n").unwrap();
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir)
        .with_label_resolution(temp_dir.path().to_path_buf(), HashMap::new());

    ctx.record_file_input(&watched).unwrap();
    ctx.record_file_input(&watched).unwrap();
    assert_eq!(ctx.recorded_inputs().unwrap().len(), 1);

    std::fs::write(&watched, "second\n").unwrap();
    let err = ctx.record_file_input(&watched).unwrap_err();
    assert!(
        err.to_string()
            .contains("Conflicting values recorded for input"),
        "{err:?}"
    );
}

#[test]
fn test_module_context_rejects_watch_outside_workspace() {
    let project_root = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let watched = outside.path().join("watched.txt");
    let working_dir = project_root.path().join("work");
    std::fs::write(&watched, "first\n").unwrap();
    std::fs::create_dir_all(&working_dir).unwrap();
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir)
        .with_label_resolution(project_root.path().to_path_buf(), HashMap::new());

    let err = ctx.record_file_input(&watched).unwrap_err();
    assert!(
        err.to_string()
            .contains("attempted to watch path outside workspace"),
        "{err:?}"
    );
}

#[test]
fn test_module_context_records_external_repo_friendly_watch_path() {
    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().join("work");
    let external_repo = temp_dir.path().join("bazel-external/rules_zig");
    let watched = external_repo.join("zig/private/versions.json");
    std::fs::create_dir_all(watched.parent().unwrap()).unwrap();
    std::fs::create_dir_all(&working_dir).unwrap();
    std::fs::write(&watched, "first\n").unwrap();
    let mut cell_paths = HashMap::new();
    cell_paths.insert("rules_zig".to_owned(), external_repo);
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir)
        .with_label_resolution(temp_dir.path().to_path_buf(), cell_paths);

    ctx.record_file_input(&watched).unwrap();

    assert_eq!(
        ctx.recorded_inputs().unwrap(),
        vec![
            slug_bzlmod::recorded_file_input_with_recorded_path(
                PathBuf::from("@@rules_zig+//zig/private/versions.json").as_path(),
                &watched,
            )
            .unwrap()
        ]
    );
}

#[test]
fn test_module_context_read_watch_parameter_records_workspace_inputs() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let temp_dir = TempDir::new().unwrap();
    let watched = temp_dir.path().join("source.txt");
    let working_dir = temp_dir.path().join("work");
    std::fs::write(&watched, "payload\n").unwrap();
    std::fs::create_dir_all(&working_dir).unwrap();
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir)
        .with_label_resolution(temp_dir.path().to_path_buf(), HashMap::new());
    let ctx_handle = ctx.clone();

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "read.star",
        format!("mctx.read({:?}, watch = 'yes')", watched.to_string_lossy()),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let result = eval.eval_module(ast, &Globals::standard()).unwrap();

    assert_eq!(result.unpack_str(), Some("payload\n"));
    assert_eq!(
        ctx_handle.recorded_inputs().unwrap(),
        vec![
            slug_bzlmod::recorded_file_input_with_recorded_path(
                PathBuf::from("@@//source.txt").as_path(),
                &watched,
            )
            .unwrap()
        ]
    );
}

#[test]
fn test_module_context_read_auto_skips_working_dir_watch() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().join("work");
    std::fs::create_dir_all(&working_dir).unwrap();
    std::fs::write(working_dir.join("generated.txt"), "generated\n").unwrap();
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir)
        .with_label_resolution(temp_dir.path().to_path_buf(), HashMap::new());
    let ctx_handle = ctx.clone();

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "read.star",
        "mctx.read('generated.txt')".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let result = eval.eval_module(ast, &Globals::standard()).unwrap();

    assert_eq!(result.unpack_str(), Some("generated\n"));
    assert_eq!(ctx_handle.recorded_inputs().unwrap(), Vec::<String>::new());
}

#[test]
fn test_module_context_read_rejects_bad_watch_value() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().join("work");
    std::fs::create_dir_all(&working_dir).unwrap();
    std::fs::write(working_dir.join("generated.txt"), "generated\n").unwrap();
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir)
        .with_label_resolution(temp_dir.path().to_path_buf(), HashMap::new());

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "read.star",
        "mctx.read('generated.txt', watch = 'maybe')".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let err = eval.eval_module(ast, &Globals::standard()).unwrap_err();

    assert!(
        err.to_string().contains("bad value for 'watch' parameter"),
        "{err:?}"
    );
}

#[test]
fn test_module_context_path_label_requires_resolver_owned_paths() {
    use starlark::environment::GlobalsBuilder;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use crate::interpreter::natives::register_bzl_module_globals;

    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().join("work");
    let legacy_repo = temp_dir.path().join("bazel-external").join("legacy_repo");
    std::fs::create_dir_all(&working_dir).unwrap();
    std::fs::create_dir_all(&legacy_repo).unwrap();
    std::fs::write(legacy_repo.join("file.txt"), "legacy\n").unwrap();
    let ctx = ModuleContext::empty().with_temp_working_dir(working_dir);

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "path.star",
        "mctx.path(Label('@legacy_repo//:file.txt'))".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let globals = GlobalsBuilder::standard()
        .with(register_bzl_module_globals)
        .build();
    let mut eval = Evaluator::new(&module);
    let err = eval.eval_module(ast, &globals).unwrap_err();

    assert!(
        err.to_string()
            .contains("requires resolver-owned bzlmod cell paths"),
        "{err:?}"
    );
}

#[test]
fn test_module_context_execute_label_requires_resolver_owned_paths() {
    use starlark::environment::GlobalsBuilder;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    use crate::interpreter::natives::register_bzl_module_globals;

    let temp_dir = TempDir::new().unwrap();
    let working_dir = temp_dir.path().join("work");
    let legacy_repo = temp_dir.path().join("bazel-external").join("legacy_repo");
    std::fs::create_dir_all(&working_dir).unwrap();
    std::fs::create_dir_all(&legacy_repo).unwrap();
    std::fs::write(legacy_repo.join("tool"), "#!/bin/sh\nexit 0\n").unwrap();
    let ctx = ModuleContext::empty().with_temp_working_dir(working_dir);

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "execute.star",
        "mctx.execute([Label('@legacy_repo//:tool')])".to_owned(),
        &Dialect::Standard,
    )
    .unwrap();
    let globals = GlobalsBuilder::standard()
        .with(register_bzl_module_globals)
        .build();
    let mut eval = Evaluator::new(&module);
    let err = eval.eval_module(ast, &globals).unwrap_err();

    assert!(
        err.to_string()
            .contains("requires resolver-owned bzlmod cell paths"),
        "{err:?}"
    );
}

fn create_module_ctx_test_tar_gz() -> Vec<u8> {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    let mut builder = tar::Builder::new(Vec::new());
    let data = b"extracted\n";
    let mut header = tar::Header::new_gnu();
    header.set_path("file.txt").unwrap();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append(&header, &data[..]).unwrap();
    let tar_data = builder.into_inner().unwrap();

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tar_data).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn test_module_context_extract_watch_archive_records_workspace_input() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let temp_dir = TempDir::new().unwrap();
    let archive = temp_dir.path().join("archive.tar.gz");
    let working_dir = temp_dir.path().join("work");
    std::fs::write(&archive, create_module_ctx_test_tar_gz()).unwrap();
    std::fs::create_dir_all(&working_dir).unwrap();
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir.clone())
        .with_label_resolution(temp_dir.path().to_path_buf(), HashMap::new());
    let ctx_handle = ctx.clone();

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "extract.star",
        format!(
            "mctx.extract({:?}, output = 'out', watch_archive = 'yes')",
            archive.to_string_lossy()
        ),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let result = eval.eval_module(ast, &Globals::standard()).unwrap();

    assert!(result.is_none());
    assert_eq!(
        std::fs::read_to_string(working_dir.join("out/file.txt")).unwrap(),
        "extracted\n"
    );
    assert_eq!(
        ctx_handle.recorded_inputs().unwrap(),
        vec![
            slug_bzlmod::recorded_file_input_with_recorded_path(
                PathBuf::from("@@//archive.tar.gz").as_path(),
                &archive,
            )
            .unwrap()
        ]
    );
}

#[test]
fn test_module_context_extract_records_archive_before_read() {
    use starlark::environment::Globals;
    use starlark::environment::Module;
    use starlark::eval::Evaluator;
    use starlark::syntax::AstModule;
    use starlark::syntax::Dialect;

    let temp_dir = TempDir::new().unwrap();
    let workspace = temp_dir.path().join("workspace");
    let working_dir = temp_dir.path().join("work");
    let archive = temp_dir.path().join("outside.tar.gz");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&working_dir).unwrap();
    let ctx = ModuleContext::empty()
        .with_temp_working_dir(working_dir)
        .with_label_resolution(workspace, HashMap::new());

    let module = Module::new();
    let heap = module.heap();
    module.set("mctx", heap.alloc(ctx));

    let ast = AstModule::parse(
        "extract.star",
        format!(
            "mctx.extract({:?}, output = 'out', watch_archive = 'yes')",
            archive.to_string_lossy()
        ),
        &Dialect::Standard,
    )
    .unwrap();
    let mut eval = Evaluator::new(&module);
    let err = eval
        .eval_module(ast, &Globals::standard())
        .expect_err("outside archive should be rejected before archive read");
    let message = err.to_string();

    assert!(message.contains("attempted to watch path outside workspace"));
    assert!(!message.contains("Failed to read archive"));
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
    let os = RepositoryOs::new_with_environ(Default::default());

    // Just verify it creates something - actual values depend on platform
    assert!(!os.name.is_empty());
    assert!(!os.arch.is_empty());
}
