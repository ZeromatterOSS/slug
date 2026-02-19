/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implementation of Bazel's `repository_rule()` built-in function.
//!
//! Plan Reference: `thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod.md` Phase 5
//!
//! ## Overview
//!
//! `repository_rule()` is used to define rules that create external repositories.
//! Unlike regular rules that define targets within a build, repository rules
//! create entire repositories that can be referenced via `@repo_name`.
//!
//! ## Example usage in Starlark:
//!
//! ```python
//! # In @bazel_tools//tools/build_defs/repo:http.bzl
//!
//! def _http_archive_impl(ctx):
//!     ctx.download_and_extract(
//!         ctx.attr.urls,
//!         ctx.attr.sha256,
//!         ctx.attr.strip_prefix,
//!     )
//!     ctx.file("WORKSPACE", "workspace(name = \"{}\")".format(ctx.name))
//!
//! http_archive = repository_rule(
//!     implementation = _http_archive_impl,
//!     attrs = {
//!         "url": attr.string(),
//!         "urls": attr.string_list(),
//!         "sha256": attr.string(),
//!         "strip_prefix": attr.string(),
//!     },
//!     environ = ["https_proxy"],
//! )
//! ```
//!
//! ## Current Status: STUB IMPLEMENTATION
//!
//! This implementation allows `repository_rule()` to be called in .bzl files
//! and stores the definition. Actual repository creation is deferred to the
//! extension execution engine.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

use allocative::Allocative;
use derive_more::Display;
use kuro_bzlmod::RepoAttrValue;
use kuro_bzlmod::RepoSpec;
use kuro_bzlmod::RepositoryInvocation;
use kuro_bzlmod::in_extension_context;
use kuro_bzlmod::record_invocation;
use kuro_bzlmod::record_repo_spec;
use starlark::any::ProvidesStaticType;
use starlark::docs::DocFunction;
use starlark::docs::DocItem;
use starlark::docs::DocMember;
use starlark::docs::DocStringKind;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::eval::ParametersSpec;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::typing::Ty;
use starlark::values::AllocValue;
use starlark::values::Freeze;
use starlark::values::FreezeError;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::dict::DictRef;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list::ListRef;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;

use crate::attrs::starlark_attribute::StarlarkAttribute;
use crate::interpreter::build_context::BuildContext;
use crate::interpreter::build_context::PerFileTypeContext;

/// Errors around repository rule declaration.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum RepositoryRuleError {
    #[error(
        "Repository rule must be assigned to a variable, e.g. `http_archive = repository_rule(...)`"
    )]
    RuleNotAssigned,
    #[error("`repository_rule` can only be declared in .bzl files")]
    RuleNotInBzl,
    #[error("Repository rule cannot be invoked before freezing")]
    RuleCalledBeforeFreezing,
    #[error("Repository rule `name` attribute is required")]
    NameAttributeRequired,
}

/// Convert a Starlark value to a RepoAttrValue for recording invocations.
fn starlark_to_repo_attr_value(value: Value) -> RepoAttrValue {
    if value.is_none() {
        return RepoAttrValue::None;
    }

    if let Some(s) = value.unpack_str() {
        // Check if it looks like a label
        if s.starts_with("//") || s.starts_with("@") || s.starts_with(":") {
            return RepoAttrValue::Label(s.to_owned());
        }
        return RepoAttrValue::String(s.to_owned());
    }

    if let Some(b) = value.unpack_bool() {
        return RepoAttrValue::Bool(b);
    }

    if let Some(i) = value.unpack_i32() {
        return RepoAttrValue::Int(i as i64);
    }

    if let Some(list) = ListRef::from_value(value) {
        let items: Vec<String> = list
            .iter()
            .filter_map(|v| v.unpack_str().map(|s| s.to_owned()))
            .collect();
        return RepoAttrValue::StringList(items);
    }

    if let Some(dict) = DictRef::from_value(value) {
        let mut map = HashMap::new();
        for (k, v) in dict.iter() {
            if let Some(key) = k.unpack_str() {
                map.insert(key.to_owned(), starlark_to_repo_attr_value(v));
            }
        }
        return RepoAttrValue::Dict(map);
    }

    // Fallback: convert to string representation
    RepoAttrValue::String(value.to_repr())
}

// ============================================================================
// StarlarkRepositoryRule - The value returned from repository_rule()
// ============================================================================

/// The value returned from a `repository_rule()` call.
/// Once frozen and called, it creates a repository.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct StarlarkRepositoryRule<'v> {
    /// The name of this rule (set when exported/assigned to a variable).
    name: RefCell<Option<String>>,
    /// The implementation function.
    /// Signature: def impl(repository_ctx) -> None
    implementation: Value<'v>,
    /// Attributes accepted by this repository rule.
    /// Stored as Vec for Trace support; converted from dict at construction.
    attrs: Vec<(String, StarlarkAttribute)>,
    /// Whether the rule is local (no remote caching).
    local: bool,
    /// Environment variables this rule depends on.
    environ: Vec<String>,
    /// Whether this rule depends on configuration settings.
    configure: bool,
    /// Whether to skip automatic creation of REPO.bazel file.
    _remotable: bool,
    /// Documentation string.
    doc: Option<String>,
    /// The bzl file path where this rule was defined (e.g. "manual_test//test_repo_ctx_simple.bzl").
    /// Used to construct the full repo_rule_id in extension context so the DICE executor
    /// can locate and execute the Starlark implementation.
    bzl_path: Option<String>,
}

impl<'v> Display for StarlarkRepositoryRule<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.name.borrow() {
            Some(name) => write!(f, "<repository_rule {}>", name),
            None => write!(f, "<unbound repository_rule>"),
        }
    }
}

impl<'v> AllocValue<'v> for StarlarkRepositoryRule<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl<'v> StarlarkRepositoryRule<'v> {
    fn new(
        implementation: Value<'v>,
        attrs: Vec<(String, StarlarkAttribute)>,
        local: bool,
        environ: Vec<String>,
        configure: bool,
        remotable: bool,
        doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<StarlarkRepositoryRule<'v>> {
        // When there's no build context (e.g. standalone/sync evaluator), allow the call.
        // When there is a context, verify we're in a .bzl file.
        let bzl_path = if let Some(build_context) =
            eval.extra.and_then(|e| e.downcast_ref::<BuildContext>())
        {
            match &build_context.additional {
                PerFileTypeContext::Bzl(bzl_ctx) => Some(bzl_ctx.bzl_path.to_string()),
                _ => return Err(RepositoryRuleError::RuleNotInBzl.into()),
            }
        } else {
            None
        };

        Ok(StarlarkRepositoryRule {
            name: RefCell::new(None),
            implementation,
            attrs,
            local,
            environ,
            configure,
            _remotable: remotable,
            doc: if doc.is_empty() {
                None
            } else {
                Some(doc.to_owned())
            },
            bzl_path,
        })
    }
}

#[starlark_value(type = "repository_rule")]
impl<'v> StarlarkValue<'v> for StarlarkRepositoryRule<'v> {
    fn export_as(
        &self,
        variable_name: &str,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<()> {
        *self.name.borrow_mut() = Some(variable_name.to_owned());
        Ok(())
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Repository rules can only be invoked after freezing
        Err(kuro_error::Error::from(RepositoryRuleError::RuleCalledBeforeFreezing).into())
    }

    fn documentation(&self) -> DocItem {
        let params = ParametersSpec::<FrozenValue>::new_named_only(
            "repository_rule",
            std::iter::empty::<(&str, _)>(),
        )
        .documentation(vec![], std::collections::HashMap::new());
        let function_docs = DocFunction::from_docstring(
            DocStringKind::Starlark,
            params,
            Ty::any(),
            self.doc.as_deref(),
        );
        DocItem::Member(DocMember::Function(function_docs))
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

/// Frozen (immutable) version of StarlarkRepositoryRule.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("<repository_rule {}>", name)]
pub struct FrozenStarlarkRepositoryRule {
    /// The name of this rule.
    name: String,
    /// The implementation function.
    implementation: FrozenValue,
    /// Attributes accepted by this repository rule.
    attrs: Vec<(String, StarlarkAttribute)>,
    /// Whether the rule is local.
    local: bool,
    /// Environment variables this rule depends on.
    environ: Vec<String>,
    /// Whether this rule depends on configuration settings.
    configure: bool,
    /// Documentation string.
    doc: Option<String>,
    /// The bzl file path where this rule was defined (e.g. "manual_test//test_repo_ctx_simple.bzl").
    bzl_path: Option<String>,
}

starlark_simple_value!(FrozenStarlarkRepositoryRule);

impl<'v> Freeze for StarlarkRepositoryRule<'v> {
    type Frozen = FrozenStarlarkRepositoryRule;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let name = match self.name.into_inner() {
            Some(name) => name,
            None => {
                return Err(FreezeError::new(
                    RepositoryRuleError::RuleNotAssigned.to_string(),
                ));
            }
        };

        Ok(FrozenStarlarkRepositoryRule {
            name,
            implementation: self.implementation.freeze(freezer)?,
            attrs: self.attrs,
            local: self.local,
            environ: self.environ,
            configure: self.configure,
            doc: self.doc,
            bzl_path: self.bzl_path,
        })
    }
}

impl FrozenStarlarkRepositoryRule {
    /// Get the name of this rule.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the implementation function.
    pub fn implementation(&self) -> FrozenValue {
        self.implementation
    }

    /// Get the attributes defined by this rule.
    pub fn attrs(&self) -> &[(String, StarlarkAttribute)] {
        &self.attrs
    }

    /// Whether this is a local rule.
    pub fn is_local(&self) -> bool {
        self.local
    }

    /// Get the environment variables this rule depends on.
    pub fn environ(&self) -> &[String] {
        &self.environ
    }

    /// Whether this rule depends on configuration settings.
    pub fn is_configure(&self) -> bool {
        self.configure
    }
}

#[starlark_value(type = "repository_rule")]
impl<'v> StarlarkValue<'v> for FrozenStarlarkRepositoryRule {
    type Canonical = StarlarkRepositoryRule<'v>;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // When a frozen repository_rule is invoked (e.g., http_archive(name = "foo", ...)),
        // we record the invocation for later execution via DICE.
        //
        // The actual repository creation happens during the DICE computation phase.

        // Parse all keyword arguments
        let kwargs = args.names_map()?;

        // The 'name' attribute is required for all repository rules
        let name = kwargs
            .get("name")
            .and_then(|v| v.unpack_str())
            .ok_or_else(|| kuro_error::Error::from(RepositoryRuleError::NameAttributeRequired))?;

        tracing::debug!(
            "Repository rule '{}' invoked with name '{}'",
            self.name,
            name,
        );

        // Check if we're in module extension execution context.
        // In extension context, we capture RepoSpecs for deferred execution.
        // Outside extension context, we record RepositoryInvocations for immediate tracking.
        if in_extension_context() {
            // Build a RepoSpec for deferred execution.
            // Use "{bzl_path}%{name}" format so the DICE executor can locate the Starlark
            // implementation. For builtin rules (no bzl_path), just use the rule name.
            let repo_rule_id = if let Some(bzl_path) = &self.bzl_path {
                format!("{}%{}", bzl_path, self.name)
            } else {
                self.name.clone()
            };
            let mut spec = RepoSpec::new(repo_rule_id);

            // Convert all kwargs (except 'name') to attributes
            for (key, value) in kwargs.iter() {
                let key_str = key.as_str();
                if key_str != "name" {
                    let attr_value = starlark_to_repo_attr_value(*value);
                    spec.attributes.insert(key_str.to_owned(), attr_value);
                }
            }

            // Record the spec in the extension registry
            record_repo_spec(name.to_owned(), spec);

            tracing::debug!(
                "Captured RepoSpec for '{}' from repository rule '{}' (extension context)",
                name,
                self.name,
            );
        } else {
            // Build the invocation record for MODULE.bazel/WORKSPACE context
            let mut invocation = RepositoryInvocation::new(name.to_owned(), self.name.clone());

            // Convert all kwargs to RepoAttrValue
            for (key, value) in kwargs.iter() {
                let attr_value = starlark_to_repo_attr_value(*value);
                invocation.attrs.insert(key.as_str().to_owned(), attr_value);
            }

            // Record the invocation in the thread-local registry
            // This will be collected after MODULE.bazel/extension parsing completes
            record_invocation(invocation);
        }

        Ok(Value::new_none())
    }

    fn documentation(&self) -> DocItem {
        let params = ParametersSpec::<FrozenValue>::new_named_only(
            &self.name,
            std::iter::empty::<(&str, _)>(),
        )
        .documentation(vec![], std::collections::HashMap::new());
        let function_docs = DocFunction::from_docstring(
            DocStringKind::Starlark,
            params,
            Ty::any(),
            self.doc.as_deref(),
        );
        DocItem::Member(DocMember::Function(function_docs))
    }

    fn get_type_starlark_repr() -> Ty {
        StarlarkRepositoryRule::get_type_starlark_repr()
    }
}

// ============================================================================
// Starlark global registration
// ============================================================================

/// Register the `repository_rule()` function as a Starlark global.
#[starlark_module]
pub fn register_repository_rule_function(builder: &mut GlobalsBuilder) {
    /// Define a repository rule.
    ///
    /// Repository rules are used to create external repositories that can be
    /// referenced via `@repo_name` in build files. They are commonly used to
    /// download external dependencies.
    ///
    /// Example:
    /// ```python
    /// def _my_repo_impl(ctx):
    ///     ctx.download(ctx.attr.url, "file.txt")
    ///     ctx.file("BUILD", "filegroup(name='all', srcs=['file.txt'])")
    ///
    /// my_repo = repository_rule(
    ///     implementation = _my_repo_impl,
    ///     attrs = {
    ///         "url": attr.string(mandatory = True),
    ///     },
    /// )
    /// ```
    ///
    /// Then in MODULE.bazel or WORKSPACE:
    /// ```python
    /// my_repo(name = "my_external_repo", url = "https://example.com/file.txt")
    /// ```
    fn repository_rule<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named, default = UnpackDictEntries::default())]
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        #[starlark(require = named, default = false)] local: bool,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        environ: UnpackListOrTuple<String>,
        #[starlark(require = named, default = false)] configure: bool,
        #[starlark(require = named, default = false)] remotable: bool,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkRepositoryRule<'v>> {
        let attrs_vec: Vec<(String, StarlarkAttribute)> = attrs
            .entries
            .into_iter()
            .map(|(name, attr)| {
                (
                    name.to_owned(),
                    StarlarkAttribute::new(attr.clone_attribute()),
                )
            })
            .collect();

        Ok(StarlarkRepositoryRule::new(
            implementation,
            attrs_vec,
            local,
            environ.items,
            configure,
            remotable,
            doc,
            eval,
        )?)
    }

    /// Type symbol for repository_rule.
    const repository_rule_type: StarlarkValueAsType<StarlarkRepositoryRule> =
        StarlarkValueAsType::new();
}
