/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Concrete implementation of Starlark repository rule execution.
//!
//! This module provides the implementation of `StarlarkRepoRuleExecutorImpl`
//! that bridges the gap between the bzlmod system and the Starlark interpreter.
//!
//! ## Architecture
//!
//! This follows the same late-binding pattern as `module_extension_executor_impl.rs`:
//!
//! ```text
//! kuro_bzlmod                             kuro_interpreter_for_build
//! ┌─────────────────────────┐             ┌──────────────────────────────────┐
//! │ ExtensionRepoExecution  │             │ ConcreteStarlarkRepoRule         │
//! │ Key                     │──late bind──│ Executor                         │
//! │                         │             │                                  │
//! │ - RepositoryInvocation  │             │ - parse_bzlmod_bzl_path()        │
//! │ - rule_source           │             │ - load .bzl via DICE             │
//! │ - working_dir           │             │ - create RepositoryContext       │
//! └─────────────────────────┘             │ - call rule.implementation(ctx)  │
//!                                         └──────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;
use dice::DiceComputations;
use kuro_bzlmod::StarlarkRepoRuleExecutorImpl;
use kuro_bzlmod::repository_invocations::RepositoryInvocation;
use kuro_common::dice::cells::HasCellResolver;
use kuro_error::BuckErrorContext;
use kuro_error::conversion::from_any_with_tag;
use kuro_interpreter::load_module::InterpreterCalculation;
use kuro_interpreter::paths::module::StarlarkModulePath;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::OwnedFrozenValueTyped;

use crate::module_extension_executor_impl::parse_bzlmod_bzl_path;
use crate::repository_ctx::AttrValue as CtxAttrValue;
use crate::repository_ctx::RepositoryAttr;
use crate::repository_ctx::RepositoryContext;
use crate::repository_rule::FrozenStarlarkRepositoryRule;

/// Errors during Starlark repository rule execution.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum StarlarkRepoRuleError {
    #[error("Repository rule '{name}' not found in module '{path}'")]
    RuleNotFound { name: String, path: String },

    #[error("Value '{name}' in '{path}' is not a repository_rule")]
    NotARepositoryRule { name: String, path: String },

    #[error("Repository rule implementation returned an error: {0}")]
    ImplementationError(String),
}

/// Convert a `kuro_bzlmod` AttrValue to a `repository_ctx` AttrValue.
fn convert_attr_value(value: &kuro_bzlmod::RepoAttrValue) -> CtxAttrValue {
    match value {
        kuro_bzlmod::RepoAttrValue::String(s) => CtxAttrValue::String(s.clone()),
        kuro_bzlmod::RepoAttrValue::Int(i) => CtxAttrValue::Int(*i),
        kuro_bzlmod::RepoAttrValue::Bool(b) => CtxAttrValue::Bool(*b),
        kuro_bzlmod::RepoAttrValue::None => CtxAttrValue::None,
        kuro_bzlmod::RepoAttrValue::StringList(list) => CtxAttrValue::StringList(list.clone()),
        kuro_bzlmod::RepoAttrValue::Label(s) => CtxAttrValue::Label(s.clone()),
        kuro_bzlmod::RepoAttrValue::Dict(map) => {
            let converted: HashMap<String, CtxAttrValue> = map
                .iter()
                .map(|(k, v)| (k.clone(), convert_attr_value(v)))
                .collect();
            CtxAttrValue::Dict(converted)
        }
    }
}

/// Convert a `CoercedAttr` default value to a `repository_ctx` AttrValue.
/// Mirrors `repository_rule::coerced_attr_to_repo_attr_value` but produces
/// the ctx flavour used directly in `RepositoryContext`.
fn coerced_attr_to_ctx_attr_value(
    attr: &kuro_node::attrs::coerced_attr::CoercedAttr,
) -> Option<CtxAttrValue> {
    use kuro_node::attrs::coerced_attr::CoercedAttr;
    match attr {
        CoercedAttr::String(s) => {
            let s = s.as_str().to_owned();
            if s.starts_with("//") || s.starts_with('@') || s.starts_with(':') {
                Some(CtxAttrValue::Label(s))
            } else {
                Some(CtxAttrValue::String(s))
            }
        }
        CoercedAttr::Int(i) => Some(CtxAttrValue::Int(*i)),
        CoercedAttr::Bool(b) => Some(CtxAttrValue::Bool(b.0)),
        CoercedAttr::None => Some(CtxAttrValue::None),
        CoercedAttr::List(list) => {
            let items: Vec<String> = list
                .iter()
                .filter_map(|v| match v {
                    CoercedAttr::String(s) => Some(s.as_str().to_owned()),
                    _ => None,
                })
                .collect();
            Some(CtxAttrValue::StringList(items))
        }
        _ => None,
    }
}

/// Concrete implementation of Starlark repository rule executor.
pub struct ConcreteStarlarkRepoRuleExecutor;

#[async_trait]
impl StarlarkRepoRuleExecutorImpl for ConcreteStarlarkRepoRuleExecutor {
    async fn execute_rule(
        &self,
        ctx: &mut DiceComputations<'_>,
        invocation: &RepositoryInvocation,
        rule_bzl_path: &str,
        rule_name: &str,
        working_dir: &Path,
    ) -> kuro_error::Result<()> {
        tracing::debug!(
            "Executing Starlark repository rule '{}' from '{}' for repo '{}'",
            rule_name,
            rule_bzl_path,
            invocation.name
        );

        // 1. Get the cell resolver to parse the bzl path
        let cell_resolver = ctx.get_cell_resolver().await?;

        // 2. Parse the bzl path into an ImportPath
        let import_path = parse_bzlmod_bzl_path(rule_bzl_path, &cell_resolver)?;

        tracing::debug!("Loading repository rule module from: {}", import_path);

        // 3. Load the module via DICE
        let loaded_module = ctx
            .get_loaded_module(StarlarkModulePath::LoadFile(&import_path))
            .await
            .buck_error_context(format!(
                "Loading repository rule bzl file: {}",
                rule_bzl_path
            ))?;

        // 4. Get the rule value from the module
        let rule_value = loaded_module
            .env()
            .get_any_visibility(rule_name)
            .map_err(|e| from_any_with_tag(e, kuro_error::ErrorTag::Input))?
            .0;

        // 5. Downcast to FrozenStarlarkRepositoryRule
        let frozen_rule: OwnedFrozenValueTyped<FrozenStarlarkRepositoryRule> = rule_value
            .downcast_starlark()
            .map_err(|_| StarlarkRepoRuleError::NotARepositoryRule {
                name: rule_name.to_owned(),
                path: rule_bzl_path.to_owned(),
            })?;

        tracing::debug!("Found repository rule '{}' in module", frozen_rule.name());

        // 6. Convert attrs from bzlmod AttrValue to repository_ctx AttrValue
        let mut ctx_attrs: HashMap<String, CtxAttrValue> = invocation
            .attrs
            .iter()
            .map(|(k, v)| (k.clone(), convert_attr_value(v)))
            .collect();

        // 6b. Merge in defaults from the rule's declared attrs for any user-
        // unspecified attribute. Matches the extension-context path in
        // repository_rule.rs:478-486.
        for (attr_name, attr_def) in frozen_rule.attrs() {
            if ctx_attrs.contains_key(attr_name) {
                continue;
            }
            if let Some(default) = attr_def.default() {
                if let Some(v) = coerced_attr_to_ctx_attr_value(default) {
                    ctx_attrs.insert(attr_name.clone(), v);
                }
            }
        }

        let repo_attr = RepositoryAttr::new_with_name(invocation.name.clone(), ctx_attrs);

        // 7. Create the RepositoryContext
        let repo_ctx = RepositoryContext::new(
            invocation.name.clone(),
            repo_attr,
            working_dir.to_path_buf(),
        );

        // 8. Execute the implementation function in Starlark
        let starlark_module = Module::new();
        let ctx_value = starlark_module.heap().alloc(repo_ctx);
        let mut eval = Evaluator::new(&starlark_module);
        let impl_fn = frozen_rule.implementation();

        tracing::debug!(
            "Invoking repository rule implementation for '{}'",
            invocation.name
        );

        let invoke_result = eval.eval_function(impl_fn.to_value(), &[ctx_value], &[]);

        match invoke_result {
            Ok(_) => {
                tracing::info!(
                    "Repository rule '{}' (rule: '{}') completed successfully",
                    invocation.name,
                    rule_name
                );
                Ok(())
            }
            Err(e) => {
                tracing::error!(
                    "Repository rule '{}' implementation failed: {}",
                    rule_name,
                    e
                );
                Err(StarlarkRepoRuleError::ImplementationError(e.to_string()).into())
            }
        }
    }
}

/// Initialize the late binding for Starlark repository rule execution.
///
/// Called from `init_late_bindings()` in lib.rs.
pub fn init_starlark_repo_rule_executor() {
    kuro_bzlmod::STARLARK_REPO_RULE_EXECUTOR_IMPL.init(&ConcreteStarlarkRepoRuleExecutor);
}
