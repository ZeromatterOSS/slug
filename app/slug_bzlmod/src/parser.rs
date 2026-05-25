/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! MODULE.bazel file parsing.
//!
//! This module provides functionality to parse MODULE.bazel files using
//! the Starlark interpreter.

use std::path::Path;
use std::path::PathBuf;

use sha2::Digest;
use sha2::Sha256;
use slug_error::BuckErrorContext;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::DialectTypes;

use crate::dice_graph::BzlmodEventKind;
use crate::dice_graph::record_bzlmod_event;
use crate::globals::ModuleFileContext;
use crate::globals::new_module_file_context;
use crate::globals::register_module_file_globals;
use crate::types::Module as BzlModule;
use crate::types::ParsedModuleFile;

/// A MODULE.bazel evaluation input and the exact bytes Slug parsed from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFileInputDigest {
    pub path: PathBuf,
    pub digest: String,
}

/// Parsed MODULE.bazel data plus every filesystem input consumed by include().
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedModuleFileWithInputs {
    pub parsed: ParsedModuleFile,
    pub inputs: Vec<ModuleFileInputDigest>,
}

/// Incremental MODULE.bazel parse/eval session for callers that own file reads.
pub struct ModuleFileParseSession {
    context: std::cell::RefCell<ModuleFileContext>,
    module_root: PathBuf,
    inputs: Vec<ModuleFileInputDigest>,
    record_events: bool,
    validate_extension_repo_directives: bool,
}

impl ModuleFileParseSession {
    pub fn new(module_root: PathBuf) -> Self {
        Self {
            context: new_module_file_context(),
            module_root,
            inputs: Vec::new(),
            record_events: true,
            validate_extension_repo_directives: true,
        }
    }

    pub fn new_silent(module_root: PathBuf) -> Self {
        Self {
            context: new_module_file_context(),
            module_root,
            inputs: Vec::new(),
            record_events: false,
            validate_extension_repo_directives: true,
        }
    }

    pub fn module_root(&self) -> &Path {
        &self.module_root
    }

    pub fn eval_segment(
        &mut self,
        path: &Path,
        content: &str,
        digest: String,
    ) -> slug_error::Result<Vec<String>> {
        self.inputs.push(ModuleFileInputDigest {
            path: path.to_path_buf(),
            digest,
        });

        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("MODULE.bazel");
        let include_start = self.context.borrow().include_labels.len();
        eval_module_bazel_content_into_context(
            content,
            filename,
            &self.context,
            self.record_events,
        )?;
        let include_labels = {
            let mut ctx = self.context.borrow_mut();
            ctx.include_labels.split_off(include_start)
        };
        Ok(include_labels)
    }

    pub fn finish(self) -> slug_error::Result<ParsedModuleFileWithInputs> {
        Ok(ParsedModuleFileWithInputs {
            parsed: parsed_module_file_from_context(
                &self.context,
                self.validate_extension_repo_directives,
            )?,
            inputs: self.inputs,
        })
    }

    pub fn allow_ignored_extension_repo_directives(mut self) -> Self {
        self.validate_extension_repo_directives = false;
        self
    }
}

/// Errors that can occur during MODULE.bazel parsing.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
pub enum ModuleParseError {
    #[error("Failed to read MODULE.bazel: {0}")]
    ReadError(String),

    #[error("Failed to parse MODULE.bazel: {0}")]
    ParseError(String),

    #[error("Failed to evaluate MODULE.bazel: {0}")]
    EvalError(String),

    #[error("Failed to include MODULE.bazel segment: {0}")]
    IncludeError(String),
}

/// The Starlark dialect for MODULE.bazel files.
fn module_bazel_dialect() -> Dialect {
    Dialect {
        // MODULE.bazel uses standard Starlark
        enable_def: true,
        enable_lambda: true,
        enable_load: false, // No load() in MODULE.bazel
        enable_keyword_only_arguments: true,
        enable_types: DialectTypes::Disable, // Types not used in MODULE.bazel
        enable_load_reexport: false,
        enable_top_level_stmt: true, // Enable variable assignments like IS_RELEASE = True
        enable_f_strings: true,
        ..Dialect::Standard
    }
}

/// Build the globals for MODULE.bazel evaluation.
fn module_bazel_globals() -> Globals {
    let mut builder = GlobalsBuilder::standard();
    register_module_file_globals(&mut builder);
    builder.build()
}

/// Parse a MODULE.bazel file from a string.
///
/// # Arguments
///
/// * `content` - The content of the MODULE.bazel file.
/// * `filename` - The filename for error messages (e.g., "MODULE.bazel").
///
/// # Returns
///
/// A `ParsedModuleFile` containing the parsed module information.
///
/// # Example
///
/// ```ignore
/// use slug_bzlmod::parse_module_bazel_content;
///
/// let content = r#"
/// module(
///     name = "my_project",
///     version = "1.0.0",
/// )
///
/// bazel_dep(name = "rules_cc", version = "0.0.9")
/// "#;
///
/// let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
/// assert_eq!(parsed.module.name, "my_project");
/// ```
pub fn parse_module_bazel_content(
    content: &str,
    filename: &str,
) -> slug_error::Result<ParsedModuleFile> {
    parse_module_bazel_content_with_options(content, filename, true)
}

pub fn parse_non_root_module_bazel_content(
    content: &str,
    filename: &str,
) -> slug_error::Result<ParsedModuleFile> {
    parse_module_bazel_content_with_options(content, filename, false)
}

fn parse_module_bazel_content_with_options(
    content: &str,
    filename: &str,
    validate_extension_repo_directives: bool,
) -> slug_error::Result<ParsedModuleFile> {
    let context = new_module_file_context();
    eval_module_bazel_content_into_context(content, filename, &context, true)?;

    if !context.borrow().include_labels.is_empty() {
        return Err(ModuleParseError::IncludeError(
            "include() requires parsing from a filesystem MODULE.bazel path".to_owned(),
        )
        .into());
    }

    parsed_module_file_from_context(&context, validate_extension_repo_directives)
}

fn eval_module_bazel_content_into_context(
    content: &str,
    filename: &str,
    context: &std::cell::RefCell<ModuleFileContext>,
    record_events: bool,
) -> slug_error::Result<()> {
    if record_events {
        record_bzlmod_event(BzlmodEventKind::ModuleFileParse, filename);
    }

    // Parse the Starlark code
    let ast = AstModule::parse(filename, content.to_owned(), &module_bazel_dialect())
        .map_err(|e| ModuleParseError::ParseError(e.to_string()))?;

    // Create evaluation environment
    let module = Module::new();
    let globals = module_bazel_globals();

    // Set up evaluator with context
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(context);

    // Evaluate the module
    eval.eval_module(ast, &globals)
        .map_err(|e| ModuleParseError::EvalError(e.to_string()))?;

    Ok(())
}

fn parsed_module_file_from_context(
    context: &std::cell::RefCell<ModuleFileContext>,
    validate_extension_repo_directives_flag: bool,
) -> slug_error::Result<ParsedModuleFile> {
    // Extract results from context
    let ctx = context.borrow();
    if validate_extension_repo_directives_flag {
        validate_extension_repo_directives(&ctx.extensions)?;
    }

    let (module_info, has_module_directive) = match &ctx.module {
        Some(decl) => {
            let mut module = BzlModule::new(decl.name.clone(), decl.version.clone());
            module.compatibility_level = decl.compatibility_level;
            module.repo_name = decl.repo_name.clone();
            module.bazel_deps = ctx.bazel_deps.clone();
            module.overrides = ctx.overrides.clone();
            (module, true)
        }
        None => {
            // No module() directive - create empty module with deps/overrides
            let mut module = BzlModule::empty();
            module.bazel_deps = ctx.bazel_deps.clone();
            module.overrides = ctx.overrides.clone();
            (module, false)
        }
    };

    Ok(ParsedModuleFile {
        module: module_info,
        has_module_directive,
        extension_usages: ctx.extensions.clone(),
        repo_rule_invocations: ctx.repo_rule_invocations.clone(),
        registered_toolchains: ctx.registered_toolchains.clone(),
        registered_execution_platforms: ctx.registered_execution_platforms.clone(),
    })
}

fn validate_extension_repo_directives(
    extension_usages: &[crate::types::ExtensionUsage],
) -> slug_error::Result<()> {
    struct ExtensionDirectiveState<'a> {
        extension_name: &'a str,
        repo_overrides: std::collections::HashMap<&'a str, &'a str>,
        injected_repos: std::collections::HashMap<&'a str, &'a str>,
    }

    let mut states = std::collections::HashMap::<String, ExtensionDirectiveState<'_>>::new();
    for ext in extension_usages {
        let state = states
            .entry(ext.extension_id())
            .or_insert_with(|| ExtensionDirectiveState {
                extension_name: &ext.extension_name,
                repo_overrides: std::collections::HashMap::new(),
                injected_repos: std::collections::HashMap::new(),
            });

        for (repo_name, overriding_repo) in
            ext.repo_overrides.iter().chain(ext.injected_repos.iter())
        {
            if let Some(previous_override) = state
                .repo_overrides
                .insert(repo_name.as_str(), overriding_repo.as_str())
            {
                return Err(ModuleParseError::EvalError(format!(
                    "The repo exported as '{}' by module extension '{}' is already overridden with '{}'",
                    repo_name, state.extension_name, previous_override
                ))
                .into());
            }
        }

        state.injected_repos.extend(
            ext.injected_repos
                .iter()
                .map(|(repo_name, overriding_repo)| (repo_name.as_str(), overriding_repo.as_str())),
        );
    }

    for ext in extension_usages {
        let Some(state) = states.get(&ext.extension_id()) else {
            continue;
        };
        for use_repo in &ext.imports {
            for repo_name in &use_repo.repos {
                if let Some(overriding_repo) = state.injected_repos.get(repo_name.as_str()) {
                    return Err(ModuleParseError::EvalError(format!(
                        "Cannot import repo '{}' that has been injected into module extension '{}'. Please refer to @{} directly.",
                        repo_name, state.extension_name, overriding_repo
                    ))
                    .into());
                }
            }
            for (_apparent_name, repo_name) in &use_repo.repo_mapping {
                if let Some(overriding_repo) = state.injected_repos.get(repo_name.as_str()) {
                    return Err(ModuleParseError::EvalError(format!(
                        "Cannot import repo '{}' that has been injected into module extension '{}'. Please refer to @{} directly.",
                        repo_name, state.extension_name, overriding_repo
                    ))
                    .into());
                }
            }
        }
    }

    Ok(())
}

/// Parse a MODULE.bazel file from a path.
///
/// # Arguments
///
/// * `path` - The path to the MODULE.bazel file.
///
/// # Returns
///
/// A `ParsedModuleFile` containing the parsed module information.
pub fn parse_module_bazel(path: &Path) -> slug_error::Result<ParsedModuleFile> {
    let content = std::fs::read_to_string(path)
        .buck_error_context(format!("Failed to read MODULE.bazel at {:?}", path))?;
    Ok(
        parse_module_bazel_content_from_path(path, &content, sha256_hex(content.as_bytes()))?
            .parsed,
    )
}

pub fn parse_non_root_module_bazel(path: &Path) -> slug_error::Result<ParsedModuleFile> {
    let content = std::fs::read_to_string(path)
        .buck_error_context(format!("Failed to read MODULE.bazel at {:?}", path))?;
    Ok(parse_non_root_module_bazel_content_from_path(
        path,
        &content,
        sha256_hex(content.as_bytes()),
    )?
    .parsed)
}

pub fn parse_module_bazel_content_from_path(
    path: &Path,
    content: &str,
    digest: String,
) -> slug_error::Result<ParsedModuleFileWithInputs> {
    parse_module_bazel_content_from_path_with_options(path, content, digest, true)
}

pub fn parse_non_root_module_bazel_content_from_path(
    path: &Path,
    content: &str,
    digest: String,
) -> slug_error::Result<ParsedModuleFileWithInputs> {
    parse_module_bazel_content_from_path_with_options(path, content, digest, false)
}

fn parse_module_bazel_content_from_path_with_options(
    path: &Path,
    content: &str,
    digest: String,
    validate_extension_repo_directives: bool,
) -> slug_error::Result<ParsedModuleFileWithInputs> {
    let module_root = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    let mut session = if validate_extension_repo_directives {
        ModuleFileParseSession::new(module_root)
    } else {
        ModuleFileParseSession::new(module_root).allow_ignored_extension_repo_directives()
    };
    let mut include_stack = Vec::new();
    let include_labels = session.eval_segment(path, content, digest)?;
    eval_module_bazel_includes_with_reader(
        &mut session,
        include_labels,
        &mut include_stack,
        &mut |include_path| {
            std::fs::read(include_path)
                .buck_error_context(format!(
                    "Failed to read included MODULE.bazel segment at {:?}",
                    include_path
                ))
                .and_then(|include_bytes| {
                    String::from_utf8(include_bytes).map_err(|e| {
                        ModuleParseError::ReadError(format!(
                            "included MODULE.bazel segment at {:?} is not UTF-8: {}",
                            include_path, e
                        ))
                        .into()
                    })
                })
        },
    )?;

    session.finish()
}

fn eval_module_bazel_includes_with_reader(
    session: &mut ModuleFileParseSession,
    include_labels: Vec<String>,
    include_stack: &mut Vec<PathBuf>,
    reader: &mut impl FnMut(&Path) -> slug_error::Result<String>,
) -> slug_error::Result<()> {
    for label in include_labels {
        let include_path = include_label_to_path(session.module_root(), &label)?;
        let canonical = include_path
            .canonicalize()
            .unwrap_or_else(|_| include_path.clone());
        if include_stack.contains(&canonical) {
            return Err(
                ModuleParseError::IncludeError(format!("cyclic include of {}", label)).into(),
            );
        }
        include_stack.push(canonical);
        let include_content = reader(&include_path)?;
        let nested_include_labels = session.eval_segment(
            &include_path,
            &include_content,
            sha256_hex(include_content.as_bytes()),
        )?;
        eval_module_bazel_includes_with_reader(
            session,
            nested_include_labels,
            include_stack,
            reader,
        )?;
        include_stack.pop();
    }

    Ok(())
}

pub fn include_label_to_path(module_root: &Path, label: &str) -> slug_error::Result<PathBuf> {
    if !label.starts_with("//") {
        return Err(ModuleParseError::IncludeError(format!(
            "bad include label '{}': include() must be called with repo-relative labels",
            label
        ))
        .into());
    }
    let without_repo = &label[2..];
    let (package, name) = without_repo.split_once(':').ok_or_else(|| {
        ModuleParseError::IncludeError(format!(
            "bad include label '{}': missing target name",
            label
        ))
    })?;
    let basename = name.rsplit('/').next().unwrap_or(name);
    if !basename.ends_with(".MODULE.bazel") || basename.starts_with('.') {
        return Err(ModuleParseError::IncludeError(format!(
            "bad include label '{}': included file must end with .MODULE.bazel and not start with '.'",
            label
        ))
        .into());
    }
    Ok(module_root.join(package).join(name))
}

fn sha256_hex(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_module() {
        let content = r#"
module(
    name = "my_project",
    version = "1.0.0",
)
"#;

        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert!(parsed.has_module_directive);
        assert_eq!(parsed.module.name, "my_project");
        assert_eq!(parsed.module.version.as_str(), "1.0.0");
        assert_eq!(parsed.module.compatibility_level, 0);
    }

    #[test]
    fn test_parse_module_bazel_expands_root_include() {
        let dir = tempfile::tempdir().unwrap();
        let root_module = dir.path().join("MODULE.bazel");
        let included = dir.path().join("deps.MODULE.bazel");

        std::fs::write(
            &root_module,
            r#"
module(name = "root")
include("//:deps.MODULE.bazel")
"#,
        )
        .unwrap();
        std::fs::write(
            &included,
            r#"
bazel_dep(name = "local_dep")
local_path_override(
    module_name = "local_dep",
    path = "local_dep",
)
"#,
        )
        .unwrap();

        let parsed = parse_module_bazel(&root_module).unwrap();
        assert_eq!(parsed.module.name, "root");
        assert_eq!(parsed.module.bazel_deps.len(), 1);
        assert_eq!(parsed.module.bazel_deps[0].name, "local_dep");
        assert_eq!(parsed.module.overrides.len(), 1);
    }

    #[test]
    fn test_module_file_parse_session_returns_include_labels_for_caller_reads() {
        let dir = tempfile::tempdir().unwrap();
        let root_module = dir.path().join("MODULE.bazel");
        let included = dir.path().join("deps.MODULE.bazel");
        let mut session = ModuleFileParseSession::new(dir.path().to_path_buf());

        let labels = session
            .eval_segment(
                &root_module,
                r#"
module(name = "root")
include("//:deps.MODULE.bazel")
"#,
                "root-digest".to_owned(),
            )
            .unwrap();
        assert_eq!(labels, vec!["//:deps.MODULE.bazel".to_owned()]);

        let nested = session
            .eval_segment(
                &included,
                r#"bazel_dep(name = "rules_cc", version = "0.2.16")"#,
                "include-digest".to_owned(),
            )
            .unwrap();
        assert!(nested.is_empty());

        let parsed = session.finish().unwrap();
        assert_eq!(parsed.inputs.len(), 2);
        assert_eq!(parsed.inputs[0].path, root_module);
        assert_eq!(parsed.inputs[1].path, included);
        assert_eq!(parsed.parsed.module.name, "root");
        assert_eq!(parsed.parsed.module.bazel_deps.len(), 1);
    }

    #[test]
    fn test_parse_module_bazel_include_variables_do_not_leak() {
        let dir = tempfile::tempdir().unwrap();
        let root_module = dir.path().join("MODULE.bazel");
        let included = dir.path().join("ext.MODULE.bazel");

        std::fs::write(
            &root_module,
            r#"
module(name = "root")
include("//:ext.MODULE.bazel")
use_repo(ext, "generated_repo")
"#,
        )
        .unwrap();
        std::fs::write(
            &included,
            r#"
ext = use_extension("//:defs.bzl", "ext")
"#,
        )
        .unwrap();

        let err = parse_module_bazel(&root_module).unwrap_err().to_string();
        assert!(err.contains("ext"), "{err}");
    }

    #[test]
    fn test_parse_module_compatibility_level_is_bazel9_noop() {
        let content = r#"
module(
    name = "my_project",
    version = "2.0.0",
    compatibility_level = 2,
)
"#;

        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert_eq!(parsed.module.compatibility_level, 0);
    }

    #[test]
    fn test_parse_module_bazel_compatibility_rejects_incompatible_version() {
        let content = r#"
module(
    name = "my_project",
    version = "2.0.0",
    bazel_compatibility = [">=99.0.0"],
)
"#;

        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("Bazel version 9.0.1 is not compatible"));
        assert!(err.contains("bazel_compatibility: [>=99.0.0]"));
    }

    #[test]
    fn test_parse_bazel_dep() {
        let content = r#"
module(name = "test", version = "1.0.0")

bazel_dep(name = "rules_cc", version = "0.0.9")
bazel_dep(name = "rules_rust", version = "0.40.0", dev_dependency = True)
"#;

        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert_eq!(parsed.module.bazel_deps.len(), 2);

        let rules_cc = &parsed.module.bazel_deps[0];
        assert_eq!(rules_cc.name, "rules_cc");
        assert_eq!(rules_cc.version.as_str(), "0.0.9");
        assert!(!rules_cc.dev_dependency);

        let rules_rust = &parsed.module.bazel_deps[1];
        assert_eq!(rules_rust.name, "rules_rust");
        assert!(rules_rust.dev_dependency);
    }

    #[test]
    fn test_parse_bazel_dep_with_repo_name() {
        let content = r#"
module(name = "test", version = "1.0.0")
bazel_dep(name = "rules_cc", version = "0.0.9", repo_name = "cc_rules")
"#;

        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        let dep = &parsed.module.bazel_deps[0];
        assert_eq!(dep.repo_name, Some("cc_rules".to_owned()));
        assert_eq!(dep.apparent_name(), "cc_rules");
    }

    #[test]
    fn test_parse_invalid_module_names_follow_bazel() {
        let cases = [
            r#"
module(name = "f.", version = "1.0.0")
"#,
            r#"
module(name = "test", version = "1.0.0")
bazel_dep(name = "Foo", version = "1.0.0")
"#,
            r#"
module(name = "test", version = "1.0.0")
local_path_override(module_name = "_dep", path = "../dep")
"#,
            r#"
module(name = "test", version = "1.0.0")
single_version_override(module_name = "dep+", version = "1.0.0")
"#,
            r#"
module(name = "test", version = "1.0.0")
multiple_version_override(module_name = "dep+", versions = ["1.0.0", "2.0.0"])
"#,
            r#"
module(name = "test", version = "1.0.0")
archive_override(module_name = "dep+", urls = ["https://example.test/dep.tar.gz"])
"#,
            r#"
module(name = "test", version = "1.0.0")
git_override(
    module_name = "dep+",
    remote = "https://example.test/dep.git",
    commit = "abc123",
)
"#,
        ];

        for content in cases {
            let err = parse_module_bazel_content(content, "MODULE.bazel")
                .unwrap_err()
                .to_string();
            assert!(err.contains("invalid module name"), "{err}");
            assert!(err.contains("valid names must"), "{err}");
        }
    }

    #[test]
    fn test_parse_invalid_user_provided_repo_names_follow_bazel() {
        let cases = [
            r#"
module(name = "test", repo_name = "_foo", version = "1.0.0")
"#,
            r#"
module(name = "test", version = "1.0.0")
bazel_dep(name = "dep", version = "1.0.0", repo_name = "_foo")
"#,
            r#"
module(name = "test", version = "1.0.0")
ext = use_extension("//:ext.bzl", "ext")
use_repo(ext, "_foo")
"#,
            r#"
module(name = "test", version = "1.0.0")
ext = use_extension("//:ext.bzl", "ext")
use_repo(ext, _foo = "foo")
"#,
            r#"
module(name = "test", version = "1.0.0")
ext = use_extension("//:ext.bzl", "ext")
use_repo(ext, foo = "_foo")
"#,
            r#"
module(name = "test", version = "1.0.0")
repo = use_repo_rule("//:repo.bzl", "repo")
repo(name = "_foo")
"#,
        ];

        for content in cases {
            let err = parse_module_bazel_content(content, "MODULE.bazel")
                .unwrap_err()
                .to_string();
            assert!(err.contains("invalid user-provided repo name"), "{err}");
            assert!(
                err.contains("must start with a letter or a number"),
                "{err}"
            );
        }
    }

    #[test]
    fn test_parse_local_path_override() {
        let content = r#"
module(name = "test", version = "1.0.0")
local_path_override(
    module_name = "my_local",
    path = "../my-local-module",
)
"#;

        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert_eq!(parsed.module.overrides.len(), 1);

        match &parsed.module.overrides[0] {
            crate::types::Override::LocalPath(o) => {
                assert_eq!(o.module_name, "my_local");
                assert_eq!(o.path, "../my-local-module");
            }
            _ => panic!("Expected LocalPath override"),
        }
    }

    #[test]
    fn test_parse_git_override() {
        let content = r#"
module(name = "test", version = "1.0.0")
git_override(
    module_name = "rules_rust",
    remote = "https://github.com/example/rules_rust.git",
    commit = "abc123",
)
"#;

        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert_eq!(parsed.module.overrides.len(), 1);

        match &parsed.module.overrides[0] {
            crate::types::Override::Git(o) => {
                assert_eq!(o.module_name, "rules_rust");
                assert_eq!(o.remote, "https://github.com/example/rules_rust.git");
                assert_eq!(o.commit, "abc123");
            }
            _ => panic!("Expected Git override"),
        }
    }

    #[test]
    fn test_parse_multiple_version_override_requires_two_versions() {
        let content = r#"
module(name = "test", version = "1.0.0")
multiple_version_override(
    module_name = "dep",
    versions = ["1.0.0"],
)
"#;

        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("multiple_version_override() must specify at least 2 versions"));
    }

    #[test]
    fn test_parse_single_version_override_with_patches_errors() {
        let content = r#"
module(name = "test", version = "1.0.0")
bazel_dep(name = "rules_rust", version = "1.0.0")
single_version_override(
    module_name = "rules_rust",
    patches = ["//:fix.patch"],
)
"#;

        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("single_version_override(patches = ...)"));
        assert!(err.contains("MODULE.bazel discovery"));
        assert!(err.contains("repository materialization"));
    }

    #[test]
    fn test_parse_non_registry_override_with_patches_errors() {
        let archive = r#"
module(name = "test", version = "1.0.0")
archive_override(
    module_name = "dep",
    urls = ["https://example.test/dep.tar.gz"],
    patches = ["//:fix.patch"],
)
"#;

        let err = parse_module_bazel_content(archive, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("archive_override(patches = ...)"));

        let git = r#"
module(name = "test", version = "1.0.0")
git_override(
    module_name = "dep",
    remote = "https://example.test/dep.git",
    commit = "abc123",
    patches = ["//:fix.patch"],
)
"#;

        let err = parse_module_bazel_content(git, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("git_override(patches = ...)"));
    }

    #[test]
    fn test_parse_override_patches_reject_external_repo_labels() {
        let single_version = r#"
module(name = "test", version = "1.0.0")
single_version_override(
    module_name = "dep",
    patches = ["@unknown_repo//:fix.patch"],
)
"#;
        let err = parse_module_bazel_content(single_version, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("only patches in the main repository can be applied"));
        assert!(err.contains("@unknown_repo"));

        let archive = r#"
module(name = "test", version = "1.0.0")
archive_override(
    module_name = "dep",
    urls = ["https://example.test/dep.tar.gz"],
    patches = ["@unknown_repo//:fix.patch"],
)
"#;
        let err = parse_module_bazel_content(archive, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("only patches in the main repository can be applied"));
        assert!(err.contains("@unknown_repo"));

        let git = r#"
module(name = "test", version = "1.0.0")
git_override(
    module_name = "dep",
    remote = "https://example.test/dep.git",
    commit = "abc123",
    patches = ["@unknown_repo//:fix.patch"],
)
"#;
        let err = parse_module_bazel_content(git, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("only patches in the main repository can be applied"));
        assert!(err.contains("@unknown_repo"));
    }

    #[test]
    fn test_parse_override_patches_treat_module_repo_name_as_main_repo() {
        let content = r#"
module(name = "test", repo_name = "root_repo", version = "1.0.0")
single_version_override(
    module_name = "dep",
    patches = ["@root_repo//:fix.patch"],
)
"#;

        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("single_version_override(patches = ...)"));
        assert!(!err.contains("only patches in the main repository can be applied"));
    }

    #[test]
    fn test_parse_single_version_override_patch_cmds_errors() {
        let content = r#"
module(name = "test", version = "1.0.0")
single_version_override(
    module_name = "dep",
    patch_cmds = ["echo patched"],
)
"#;

        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("single_version_override(patch_cmds = ...)"));
        assert!(err.contains("final repo spec"));
    }

    #[test]
    fn test_parse_single_version_override_patch_strip_errors() {
        let content = r#"
module(name = "test", version = "1.0.0")
single_version_override(
    module_name = "dep",
    patch_strip = 1,
)
"#;

        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("single_version_override(patch_strip = ...)"));
        assert!(err.contains("patch_args"));
        assert!(err.contains("final repo spec"));
    }

    #[test]
    fn test_parse_no_module_directive() {
        let content = r#"
bazel_dep(name = "rules_cc", version = "0.0.9")
"#;

        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert!(!parsed.has_module_directive);
        assert!(parsed.module.name.is_empty());
        assert_eq!(parsed.module.bazel_deps.len(), 1);
    }

    #[test]
    fn test_parse_empty_file() {
        let content = "";
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert!(!parsed.has_module_directive);
        assert!(parsed.module.bazel_deps.is_empty());
    }

    #[test]
    fn test_parse_syntax_error() {
        let content = "this is not valid starlark [[[";
        let result = parse_module_bazel_content(content, "MODULE.bazel");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_multiple_module_calls() {
        let content = r#"
module(name = "first", version = "1.0.0")
module(name = "second", version = "2.0.0")
"#;
        let result = parse_module_bazel_content(content, "MODULE.bazel");
        assert!(result.is_err());
    }

    // ========================================================================
    // Extension Parsing Tests (Phase 5)
    // ========================================================================

    #[test]
    fn test_parse_use_extension_basic() {
        let content = r#"
module(name = "test", version = "1.0.0")
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert_eq!(parsed.extension_usages.len(), 1);

        let ext = &parsed.extension_usages[0];
        assert_eq!(
            ext.extension_bzl_file,
            "@rules_python//python/extensions:pip.bzl"
        );
        assert_eq!(ext.extension_name, "pip");
        assert!(!ext.dev_dependency);
        assert!(!ext.isolate);
    }

    #[test]
    fn test_parse_use_extension_with_dev_dependency() {
        let content = r#"
module(name = "test", version = "1.0.0")
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip", dev_dependency = True)
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        let ext = &parsed.extension_usages[0];
        assert!(ext.dev_dependency);
    }

    #[test]
    fn test_parse_use_extension_with_isolate_errors() {
        let content = r#"
module(name = "test", version = "1.0.0")
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip", isolate = True)
"#;
        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("use_extension(isolate = True)"));
        assert!(err.contains("experimental_isolated_extension_usages"));
    }

    #[test]
    fn test_parse_use_repo_rule_with_dev_dependency() {
        let content = r#"
module(name = "test", version = "1.0.0")
repo = use_repo_rule("@@bazel_tools//tools/build_defs/repo:local.bzl", "local_repository", dev_dependency = True)
repo(name = "dev_repo", path = "dev_repo")
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        let invocation = &parsed.repo_rule_invocations[0];
        assert_eq!(invocation.name, "dev_repo");
        assert!(invocation.dev_dependency);
    }

    #[test]
    fn test_parse_use_extension_with_tags() {
        let content = r#"
module(name = "test", version = "1.0.0")
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
pip.parse(
    hub_name = "pip",
    python_version = "3.11",
    requirements_lock = "//:requirements_lock.txt",
)
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        let ext = &parsed.extension_usages[0];
        assert_eq!(ext.tags.len(), 1);

        let tag = &ext.tags[0];
        assert_eq!(tag.tag_name, "parse");
        assert_eq!(tag.kwargs.len(), 3);

        // Check kwargs
        let kwargs_map: std::collections::HashMap<_, _> =
            tag.kwargs.iter().map(|(k, v)| (k.as_str(), v)).collect();

        assert!(matches!(
            kwargs_map.get("hub_name"),
            Some(crate::types::TagValue::String(s)) if s == "pip"
        ));
        // Plan 10 Phase 4: relative `//`-labels in a non-root module
        // canonicalize to `@@<owning_module>//pkg:target`. The MODULE.bazel
        // above declares `module(name = "test")`, so this label canonicalizes
        // to `@@test//:requirements_lock.txt`.
        assert!(matches!(
            kwargs_map.get("requirements_lock"),
            Some(crate::types::TagValue::Label(s)) if s == "@@test//:requirements_lock.txt"
        ));
    }

    #[test]
    fn test_parse_use_extension_with_use_repo() {
        let content = r#"
module(name = "test", version = "1.0.0")
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
pip.parse(hub_name = "pip")
use_repo(pip, "pip", "pip_internal")
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        let ext = &parsed.extension_usages[0];
        assert_eq!(ext.imports.len(), 1);

        let use_repo = &ext.imports[0];
        assert_eq!(use_repo.repos.len(), 2);
        assert_eq!(use_repo.repos[0], "pip");
        assert_eq!(use_repo.repos[1], "pip_internal");
    }

    #[test]
    fn test_parse_override_repo_positional_and_keyword() {
        let content = r#"
module(name = "test", version = "1.0.0")
ext = use_extension("//:ext.bzl", "ext")
override_repo(ext, "generated", public = "replacement")
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        let ext = &parsed.extension_usages[0];
        assert_eq!(
            ext.repo_overrides,
            vec![
                ("generated".to_owned(), "generated".to_owned()),
                ("public".to_owned(), "replacement".to_owned()),
            ]
        );
    }

    #[test]
    fn test_parse_duplicate_override_repo_rows_error() {
        let content = r#"
module(name = "test", version = "1.0.0")
ext = use_extension("//:ext.bzl", "ext")
override_repo(ext, "generated")
override_repo(ext, generated = "replacement")
"#;
        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("repo exported as 'generated'"));
        assert!(err.contains("module extension 'ext'"));
        assert!(err.contains("already overridden"));
    }

    #[test]
    fn test_parse_duplicate_override_and_inject_repo_rows_error() {
        let content = r#"
module(name = "test", version = "1.0.0")
ext = use_extension("//:ext.bzl", "ext")
override_repo(ext, generated = "replacement")
inject_repo(ext, generated = "helper")
"#;
        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("repo exported as 'generated'"));
        assert!(err.contains("module extension 'ext'"));
        assert!(err.contains("already overridden"));
    }

    #[test]
    fn test_parse_duplicate_override_repo_across_same_extension_proxies_errors() {
        let content = r#"
module(name = "test", version = "1.0.0")
first = use_extension("//:ext.bzl", "ext")
second = use_extension("//:ext.bzl", "ext")
override_repo(first, generated = "replacement")
inject_repo(second, generated = "helper")
"#;
        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("repo exported as 'generated'"));
        assert!(err.contains("module extension 'ext'"));
        assert!(err.contains("already overridden"));
    }

    #[test]
    fn test_parse_use_repo_of_injected_repo_errors() {
        let content = r#"
module(name = "test", version = "1.0.0")
ext = use_extension("//:ext.bzl", "ext")
use_repo(ext, "generated")
inject_repo(ext, generated = "helper")
"#;
        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("Cannot import repo 'generated'"));
        assert!(err.contains("has been injected into module extension 'ext'"));
        assert!(err.contains("Please refer to @helper directly"));
    }

    #[test]
    fn test_parse_non_root_use_repo_of_injected_repo_allows_ignored_inject() {
        let content = r#"
module(name = "test", version = "1.0.0")
ext = use_extension("//:ext.bzl", "ext")
use_repo(ext, "generated")
inject_repo(ext, generated = "helper")
"#;
        let parsed = parse_non_root_module_bazel_content(content, "MODULE.bazel").unwrap();

        assert_eq!(parsed.extension_usages.len(), 1);
        assert_eq!(
            parsed.extension_usages[0].injected_repos,
            vec![("generated".to_owned(), "helper".to_owned())]
        );
    }

    #[test]
    fn test_parse_use_repo_of_injected_repo_across_same_extension_proxies_errors() {
        let content = r#"
module(name = "test", version = "1.0.0")
first = use_extension("//:ext.bzl", "ext")
second = use_extension("//:ext.bzl", "ext")
use_repo(first, alias = "generated")
inject_repo(second, generated = "helper")
"#;
        let err = parse_module_bazel_content(content, "MODULE.bazel")
            .unwrap_err()
            .to_string();
        assert!(err.contains("Cannot import repo 'generated'"));
        assert!(err.contains("has been injected into module extension 'ext'"));
        assert!(err.contains("Please refer to @helper directly"));
    }

    #[test]
    fn test_parse_multiple_extensions() {
        let content = r#"
module(name = "test", version = "1.0.0")
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
pip.parse(hub_name = "pip")
use_repo(pip, "pip")

maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(artifacts = ["com.google.guava:guava:31.1-jre"])
use_repo(maven, "maven")
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert_eq!(parsed.extension_usages.len(), 2);

        assert_eq!(parsed.extension_usages[0].extension_name, "pip");
        assert_eq!(parsed.extension_usages[1].extension_name, "maven");
    }

    #[test]
    fn test_parse_extension_tag_with_list() {
        let content = r#"
module(name = "test", version = "1.0.0")
maven = use_extension("@rules_jvm_external//:extensions.bzl", "maven")
maven.install(artifacts = ["guava", "protobuf"])
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        let tag = &parsed.extension_usages[0].tags[0];

        let artifacts = tag
            .kwargs
            .iter()
            .find(|(k, _)| k == "artifacts")
            .map(|(_, v)| v);
        assert!(matches!(artifacts, Some(crate::types::TagValue::List(items)) if items.len() == 2));
    }

    #[test]
    fn test_parse_extension_tag_with_bool() {
        let content = r#"
module(name = "test", version = "1.0.0")
pip = use_extension("@rules_python//python/extensions:pip.bzl", "pip")
pip.parse(quiet = False)
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        let tag = &parsed.extension_usages[0].tags[0];

        let quiet = tag
            .kwargs
            .iter()
            .find(|(k, _)| k == "quiet")
            .map(|(_, v)| v);
        assert!(matches!(quiet, Some(crate::types::TagValue::Bool(false))));
    }

    #[test]
    fn test_parse_bazel_lib_style_no_arg_tags() {
        // Test MODULE.bazel style from bazel_lib which uses no-argument tag calls
        let content = r#"
module(
    name = "bazel_lib",
    version = "3.1.1",
)

bazel_dep(name = "bazel_skylib", version = "1.8.1")
bazel_dep(name = "platforms", version = "0.0.10")

bazel_lib_toolchains = use_extension("@bazel_lib//lib:extensions.bzl", "toolchains")
bazel_lib_toolchains.copy_directory()
bazel_lib_toolchains.copy_to_directory()
bazel_lib_toolchains.coreutils()
use_repo(bazel_lib_toolchains, "copy_directory_toolchains", "copy_to_directory_toolchains")
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();

        // Should have one extension usage
        assert_eq!(parsed.extension_usages.len(), 1);

        let ext = &parsed.extension_usages[0];
        assert_eq!(ext.extension_bzl_file, "@bazel_lib//lib:extensions.bzl");
        assert_eq!(ext.extension_name, "toolchains");

        // Should have three tags (no-argument calls)
        assert_eq!(ext.tags.len(), 3);
        assert_eq!(ext.tags[0].tag_name, "copy_directory");
        assert_eq!(ext.tags[1].tag_name, "copy_to_directory");
        assert_eq!(ext.tags[2].tag_name, "coreutils");

        // Each tag should have no kwargs
        for tag in &ext.tags {
            assert!(tag.kwargs.is_empty());
        }
    }

    #[test]
    fn test_parse_bazel_lib_actual_from_bcr() {
        // Test actual bazel_lib MODULE.bazel content from BCR which uses variable assignments
        let content = r#"
module(
    name = "bazel_lib",
    bazel_compatibility = [">=6.0.0"],
    compatibility_level = 1,
    version = "3.1.1",
)

bazel_dep(name = "bazel_features", version = "1.9.0")
bazel_dep(name = "bazel_skylib", version = "1.8.1")
bazel_dep(name = "platforms", version = "0.0.10")
bazel_dep(name = "rules_shell", version = "0.4.1")

bazel_lib_toolchains = use_extension("@bazel_lib//lib:extensions.bzl", "toolchains")
bazel_lib_toolchains.copy_directory()
bazel_lib_toolchains.copy_to_directory()
bazel_lib_toolchains.coreutils()
bazel_lib_toolchains.zstd()
bazel_lib_toolchains.expand_template()
bazel_lib_toolchains.bats()
use_repo(bazel_lib_toolchains, "bats_toolchains", "copy_directory_toolchains", "copy_to_directory_toolchains")

register_toolchains(
    "@copy_directory_toolchains//:all",
    "@copy_to_directory_toolchains//:all",
)

# Variable assignment - this was causing the parse failure!
IS_RELEASE = True

bazel_dep(
    name = "gazelle",
    version = "0.40.0",
    dev_dependency = IS_RELEASE,
)
bazel_dep(
    name = "rules_go",
    version = "0.59.0",
    dev_dependency = IS_RELEASE,
    repo_name = "io_bazel_rules_go",
)
"#;
        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert_eq!(parsed.module.name, "bazel_lib");
        assert_eq!(parsed.module.compatibility_level, 0);

        // Should have multiple bazel_deps including dev dependencies
        assert!(parsed.module.bazel_deps.len() >= 6);

        // Should have the toolchains extension
        assert!(!parsed.extension_usages.is_empty());
        let toolchains_ext = parsed
            .extension_usages
            .iter()
            .find(|e| e.extension_name == "toolchains")
            .expect("Should have toolchains extension");
        assert_eq!(toolchains_ext.tags.len(), 6);
    }
}
