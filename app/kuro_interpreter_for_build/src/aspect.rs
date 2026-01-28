/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implementation of Bazel's `aspect()` built-in function.
//!
//! Plan Reference: `thoughts/shared/plans/kuro-bazel-subplans/08-aspects.md`
//!
//! ## Current Status: STUB IMPLEMENTATION (Phase 8a)
//!
//! This is a minimal stub that allows aspect() to be called and returns an Aspect
//! object. The full Bazel semantics are NOT yet implemented.
//!
//! ## What This Stub Does
//!
//! - Allows `aspect()` calls to parse without error
//! - Returns a placeholder that can be attached to attributes
//! - The aspect implementation function is NOT called (stub only)
//!
//! ## Missing Features (TODO - Phases 8b-8d)
//!
//! Per the Bazel documentation and our plan, the following need to be implemented:
//!
//! 1. **AspectContext**: A context object passed to aspect implementation:
//!    - `ctx.rule.kind` - the kind of rule being visited
//!    - `ctx.rule.attr` - the rule's attributes (resolved to aspect results)
//!    - `ctx.label` - target label
//!    - `ctx.actions` - action registration
//!    - `ctx.attr` - aspect-specific attributes
//!
//! 2. **Shadow Graph Propagation**: When aspect A is applied to target X:
//!    - First apply A to all dependencies reachable via `attr_aspects`
//!    - In A(X), replace deps with aspect results
//!
//! 3. **DICE Integration**: Cache aspect computations incrementally
//!
//! 4. **Advanced Features**:
//!    - `required_aspect_providers` - access providers from other aspects
//!    - `requires` - declare aspect dependencies
//!    - `toolchains` - toolchain resolution for aspects
//!
//! ## Example usage in Starlark:
//!
//! ```python
//! def _my_aspect_impl(target, ctx):
//!     # target - the target this aspect is applied to
//!     # ctx - aspect context with ctx.rule.kind, ctx.rule.attr, etc.
//!     if SomeInfo in target:
//!         # Process the target's providers
//!         pass
//!     return [MyAspectInfo(...)]
//!
//! my_aspect = aspect(
//!     implementation = _my_aspect_impl,
//!     attr_aspects = ["deps"],  # Propagate through deps attribute
//!     required_providers = [[SomeInfo]],  # Only apply to targets with SomeInfo
//!     attrs = {
//!         "_tool": attr.label(default = "//tools:my_tool"),
//!     },
//! )
//!
//! # Then in a rule's attribute:
//! my_rule = rule(
//!     implementation = _rule_impl,
//!     attrs = {
//!         "deps": attr.label_list(aspects = [my_aspect]),
//!     },
//! )
//! ```

use std::cell::RefCell;
use std::fmt;

use allocative::Allocative;
use derive_more::Display;
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
use starlark::typing::ParamSpec;
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
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;

use crate::attrs::starlark_attribute::StarlarkAttribute;
use crate::interpreter::build_context::BuildContext;
use crate::interpreter::build_context::PerFileTypeContext;

/// Errors around aspect declaration and invocation.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum AspectError {
    #[error(
        "Aspect must be assigned to a variable before use, e.g. `my_aspect = aspect(...)`"
    )]
    AspectNotAssigned,
    #[error("`aspect` can only be declared in .bzl files")]
    AspectNotInBzl,
    #[error("Aspect cannot be invoked directly - it must be attached to an attribute via `aspects = [...]`")]
    AspectCannotBeInvokedDirectly,
}

/// The callable that's returned from an `aspect()` call. This is a stub implementation
/// that allows aspects to be declared and attached to attributes, but doesn't execute
/// the aspect implementation function.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct StarlarkAspectCallable<'v> {
    /// The name of this aspect (set when exported/assigned to a variable).
    name: RefCell<Option<String>>,
    /// The implementation function for this aspect.
    /// Signature: def impl(target, ctx) -> list[Provider]
    implementation: Value<'v>,
    /// Attributes through which this aspect propagates (e.g., ["deps", "srcs"]).
    /// Use ["*"] to propagate through all label/label_list attributes.
    attr_aspects: Vec<String>,
    /// Aspect-specific attributes (usually private, starting with "_").
    attrs: Vec<(String, StarlarkAttribute)>,
    /// Filter: only apply to targets providing these providers.
    /// Outer list is OR, inner list is AND.
    /// E.g., [[FooInfo], [BarInfo, BazInfo]] means: FooInfo OR (BarInfo AND BazInfo)
    required_providers: Vec<Vec<Value<'v>>>,
    /// Access providers from other aspects that have already run.
    required_aspect_providers: Vec<Vec<Value<'v>>>,
    /// Providers that this aspect declares it will return.
    provides: Vec<Value<'v>>,
    /// Other aspects that must run before this one.
    requires: Vec<Value<'v>>,
    /// Configuration fragments this aspect needs access to.
    fragments: Vec<String>,
    /// Toolchains this aspect requires.
    toolchains: Vec<String>,
    /// Documentation string.
    doc: Option<String>,
    /// Apply to generating rule of output files.
    apply_to_generating_rules: bool,
    /// Execution platform constraints.
    exec_compatible_with: Vec<String>,
    /// Subrules used by this aspect.
    subrules: Vec<Value<'v>>,
}

impl<'v> Display for StarlarkAspectCallable<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.name.borrow() {
            Some(name) => write!(f, "<aspect {}>", name),
            None => write!(f, "<unbound aspect>"),
        }
    }
}

impl<'v> AllocValue<'v> for StarlarkAspectCallable<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl<'v> StarlarkAspectCallable<'v> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        implementation: Value<'v>,
        attr_aspects: Vec<String>,
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        required_providers: Vec<Vec<Value<'v>>>,
        required_aspect_providers: Vec<Vec<Value<'v>>>,
        provides: Vec<Value<'v>>,
        requires: Vec<Value<'v>>,
        fragments: Vec<String>,
        toolchains: Vec<String>,
        doc: &str,
        apply_to_generating_rules: bool,
        exec_compatible_with: Vec<String>,
        subrules: Vec<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<StarlarkAspectCallable<'v>> {
        let build_context = BuildContext::from_context(eval)?;

        // Verify we're in a .bzl file
        match &build_context.additional {
            PerFileTypeContext::Bzl(_) => {}
            _ => return Err(AspectError::AspectNotInBzl.into()),
        }

        let attrs_vec: Vec<(String, StarlarkAttribute)> = attrs
            .entries
            .into_iter()
            .map(|(name, attr)| (name.to_owned(), StarlarkAttribute::new(attr.clone_attribute())))
            .collect();

        Ok(StarlarkAspectCallable {
            name: RefCell::new(None),
            implementation,
            attr_aspects,
            attrs: attrs_vec,
            required_providers,
            required_aspect_providers,
            provides,
            requires,
            fragments,
            toolchains,
            doc: if doc.is_empty() {
                None
            } else {
                Some(doc.to_owned())
            },
            apply_to_generating_rules,
            exec_compatible_with,
            subrules,
        })
    }
}

#[starlark_value(type = "Aspect")]
impl<'v> StarlarkValue<'v> for StarlarkAspectCallable<'v> {
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
        // Aspects cannot be called directly - they are applied to targets via attributes
        Err(kuro_error::Error::from(AspectError::AspectCannotBeInvokedDirectly).into())
    }

    fn documentation(&self) -> DocItem {
        let params = ParametersSpec::<FrozenValue>::new_named_only(
            "aspect",
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
        // Aspects are not callable, but we need a type representation
        Ty::function(ParamSpec::kwargs(Ty::any()), Ty::any())
    }
}

/// Frozen (immutable) version of StarlarkAspectCallable.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("<aspect {}>", name)]
pub struct FrozenStarlarkAspectCallable {
    /// The name of this aspect.
    name: String,
    /// The implementation function for this aspect.
    implementation: FrozenValue,
    /// Attributes through which this aspect propagates.
    attr_aspects: Vec<String>,
    /// Aspect-specific attributes.
    attrs: Vec<(String, StarlarkAttribute)>,
    /// Configuration fragments this aspect needs access to.
    fragments: Vec<String>,
    /// Toolchains this aspect requires.
    toolchains: Vec<String>,
    /// Documentation string.
    doc: Option<String>,
    /// Apply to generating rule of output files.
    apply_to_generating_rules: bool,
}

starlark_simple_value!(FrozenStarlarkAspectCallable);

impl<'v> Freeze for StarlarkAspectCallable<'v> {
    type Frozen = FrozenStarlarkAspectCallable;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let name = match self.name.into_inner() {
            Some(name) => name,
            None => {
                return Err(FreezeError::new(AspectError::AspectNotAssigned.to_string()));
            }
        };

        Ok(FrozenStarlarkAspectCallable {
            name,
            implementation: self.implementation.freeze(freezer)?,
            attr_aspects: self.attr_aspects,
            attrs: self.attrs,
            fragments: self.fragments,
            toolchains: self.toolchains,
            doc: self.doc,
            apply_to_generating_rules: self.apply_to_generating_rules,
        })
    }
}

impl FrozenStarlarkAspectCallable {
    /// Get the name of this aspect.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the attributes defined by this aspect.
    pub fn attrs(&self) -> &[(String, StarlarkAttribute)] {
        &self.attrs
    }

    /// Get the attributes through which this aspect propagates.
    pub fn attr_aspects(&self) -> &[String] {
        &self.attr_aspects
    }

    /// Get the fragments this aspect requires.
    pub fn fragments(&self) -> &[String] {
        &self.fragments
    }

    /// Get the toolchains this aspect requires.
    pub fn toolchains(&self) -> &[String] {
        &self.toolchains
    }

    /// Get the implementation function.
    pub fn implementation(&self) -> FrozenValue {
        self.implementation
    }

    /// Get whether this aspect applies to generating rules of output files.
    pub fn apply_to_generating_rules(&self) -> bool {
        self.apply_to_generating_rules
    }
}

#[starlark_value(type = "Aspect")]
impl<'v> StarlarkValue<'v> for FrozenStarlarkAspectCallable {
    type Canonical = StarlarkAspectCallable<'v>;

    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Aspects cannot be called directly - they are applied to targets via attributes
        Err(kuro_error::Error::from(AspectError::AspectCannotBeInvokedDirectly).into())
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
        StarlarkAspectCallable::get_type_starlark_repr()
    }
}

/// Helper to parse required_providers which can be a flat list or nested list.
/// Bazel syntax:
/// - `[FooInfo, BarInfo]` - flat list, all required
/// - `[[FooInfo], [BarInfo]]` - nested list, any-of (OR)
fn parse_required_providers<'v>(
    providers: UnpackListOrTuple<Value<'v>>,
) -> Vec<Vec<Value<'v>>> {
    let mut result = Vec::new();
    for v in providers.items {
        // Check if this element is itself a list
        if let Some(list_ref) = starlark::values::list::ListRef::from_value(v) {
            // It's a nested list - treat as one OR clause
            result.push(list_ref.iter().collect());
        } else {
            // It's a single provider - wrap in a single-element list
            result.push(vec![v]);
        }
    }
    result
}

/// Register the `aspect()` function as a Starlark global.
#[starlark_module]
pub fn register_aspect_function(builder: &mut GlobalsBuilder) {
    /// Define an aspect - a mechanism to traverse the dependency graph and
    /// collect information or perform additional actions on targets.
    ///
    /// Aspects allow additional computation to run over a target's dependency graph.
    /// When an aspect is attached to a dependency attribute, it automatically propagates
    /// through the graph, creating a "shadow graph" where each node runs the aspect's
    /// implementation function.
    ///
    /// Example:
    /// ```python
    /// def _my_aspect_impl(target, ctx):
    ///     # target - the target this aspect is applied to
    ///     # ctx - aspect context with ctx.rule.kind, ctx.rule.attr, etc.
    ///     if SomeInfo in target:
    ///         info = target[SomeInfo]
    ///         # Process the target's providers
    ///     return [MyAspectInfo(data = collected_data)]
    ///
    /// my_aspect = aspect(
    ///     implementation = _my_aspect_impl,
    ///     attr_aspects = ["deps"],  # Propagate through deps attribute
    ///     required_providers = [[SomeInfo]],  # Only apply to targets with SomeInfo
    /// )
    /// ```
    ///
    /// NOTE: This is currently a stub implementation (Phase 8a). The aspect can be
    /// defined and attached to attributes, but the implementation function is not
    /// yet called during analysis.
    fn aspect<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        attr_aspects: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackDictEntries::default())]
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        required_providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        required_aspect_providers: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        provides: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        requires: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        fragments: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        toolchains: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        #[starlark(require = named, default = false)] apply_to_generating_rules: bool,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        exec_compatible_with: UnpackListOrTuple<String>,
        #[starlark(require = named)] exec_groups: Option<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        subrules: UnpackListOrTuple<Value<'v>>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkAspectCallable<'v>> {
        // exec_groups is accepted but ignored for now
        let _unused = exec_groups;

        // Convert toolchains to strings (they're typically labels but we store as strings for now)
        let toolchains_strings: Vec<String> = toolchains.items.iter().map(|v| v.to_str()).collect();

        // Parse required_providers (can be flat or nested list)
        let required_providers_parsed = parse_required_providers(required_providers);
        let required_aspect_providers_parsed = parse_required_providers(required_aspect_providers);

        Ok(StarlarkAspectCallable::new(
            implementation,
            attr_aspects.items,
            attrs,
            required_providers_parsed,
            required_aspect_providers_parsed,
            provides.items,
            requires.items,
            fragments.items,
            toolchains_strings,
            doc,
            apply_to_generating_rules,
            exec_compatible_with.items,
            subrules.items,
            eval,
        )?)
    }

    /// Type symbol for Aspect.
    const Aspect: StarlarkValueAsType<StarlarkAspectCallable> = StarlarkValueAsType::new();
}
