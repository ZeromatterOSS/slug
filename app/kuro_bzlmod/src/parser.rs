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

use kuro_error::BuckErrorContext;
use starlark::environment::Globals;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark::syntax::DialectTypes;

use crate::globals::new_module_file_context;
use crate::globals::register_module_file_globals;
use crate::types::Module as BzlModule;
use crate::types::ParsedModuleFile;

/// Errors that can occur during MODULE.bazel parsing.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
pub enum ModuleParseError {
    #[error("Failed to read MODULE.bazel: {0}")]
    ReadError(String),

    #[error("Failed to parse MODULE.bazel: {0}")]
    ParseError(String),

    #[error("Failed to evaluate MODULE.bazel: {0}")]
    EvalError(String),
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
        enable_top_level_stmt: false,
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
/// use kuro_bzlmod::parser::parse_module_bazel_content;
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
) -> kuro_error::Result<ParsedModuleFile> {
    // Parse the Starlark code
    let ast = AstModule::parse(filename, content.to_owned(), &module_bazel_dialect())
        .map_err(|e| ModuleParseError::ParseError(e.to_string()))?;

    // Create evaluation environment
    let module = Module::new();
    let globals = module_bazel_globals();
    let context = new_module_file_context();

    // Set up evaluator with context
    let mut eval = Evaluator::new(&module);
    eval.extra = Some(&context);

    // Evaluate the module
    eval.eval_module(ast, &globals)
        .map_err(|e| ModuleParseError::EvalError(e.to_string()))?;

    // Extract results from context
    let ctx = context.borrow();

    let (module_info, has_module_directive) = match &ctx.module {
        Some(decl) => {
            let mut module = BzlModule::new(decl.name.clone(), decl.version.clone());
            module.compatibility_level = decl.compatibility_level;
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
    })
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
pub fn parse_module_bazel(path: &Path) -> kuro_error::Result<ParsedModuleFile> {
    let content = std::fs::read_to_string(path)
        .buck_error_context(format!("Failed to read MODULE.bazel at {:?}", path))?;

    let filename = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("MODULE.bazel");

    parse_module_bazel_content(&content, filename)
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
    fn test_parse_module_with_compatibility_level() {
        let content = r#"
module(
    name = "my_project",
    version = "2.0.0",
    compatibility_level = 2,
)
"#;

        let parsed = parse_module_bazel_content(content, "MODULE.bazel").unwrap();
        assert_eq!(parsed.module.compatibility_level, 2);
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
}
