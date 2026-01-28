/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implementation of Bazel's `subrule()` built-in function.
//!
//! Design Reference: `thoughts/shared/research/bazel-subrule-design.md`
//! (Original: https://docs.google.com/document/d/1RbNC88QieKvBEwir7iV5zZU08AaMlOzxhVkPnmKDedQ)
//!
//! ## Current Status: STUB IMPLEMENTATION
//!
//! This is a minimal stub that allows subrule() to be called and returns a Subrule
//! object. The full Bazel semantics are NOT yet implemented.
//!
//! ## Missing Features (TODO)
//!
//! Per the Bazel design document, the following need to be implemented:
//!
//! 1. **SubruleContext**: A stripped-down context object passed to implementation:
//!    - `ctx.actions` - for creating actions (with implicit toolchain/exec_group)
//!    - `ctx.toolchains` - for accessing declared toolchains
//!    - `ctx.label` - target label for naming artifacts
//!    - Possibly `ctx.fragments` for configuration access
//!    - NOT provided: attr, file, files, executable, bin_dir, etc.
//!
//! 2. **Attribute lifting**: When a rule declares `subrules=[my_subrule]`:
//!    - Subrule's implicit deps (attrs starting with `_`) are lifted to the rule
//!    - All attrs must be `attr.label` or `attr.label_list` only
//!    - No public attrs allowed on subrules
//!
//! 3. **Call semantics**: When subrule is invoked from rule implementation:
//!    - First positional arg is SubruleContext (not RuleContext)
//!    - Implicit deps passed as keyword arguments
//!    - Error if subrule called but not declared in `subrules=[]`
//!    - Can return arbitrary values (not limited to providers)
//!
//! 4. **rule() and aspect() changes**: Add `subrules` parameter
//!
//! 5. **Toolchain/exec_group support**: Subrules can declare their own toolchains
//!
//! ## Example usage in Starlark:
//!
//! ```python
//! def _my_subrule_impl(ctx, *, _some_attr):
//!     # ctx is SubruleContext (NOT RuleContext)
//!     # _some_attr is the resolved implicit dependency
//!     return struct(result = "computed value")
//!
//! my_subrule = subrule(
//!     implementation = _my_subrule_impl,
//!     attrs = {
//!         "_some_attr": attr.label(default = "//some:target"),
//!     },
//!     toolchains = ["//my:toolchain_type"],
//! )
//!
//! # Then in a rule:
//! def _rule_impl(ctx):
//!     result = my_subrule(source_files=ctx.files.srcs)  # Called directly!
//!     return [DefaultInfo()]
//!
//! my_rule = rule(
//!     implementation = _rule_impl,
//!     attrs = {"srcs": attr.label_list()},
//!     subrules = [my_subrule],  # Must declare subrules used
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
use starlark::values::ValueLike;
use starlark::values::dict::UnpackDictEntries;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;

use crate::attrs::starlark_attribute::StarlarkAttribute;
use crate::interpreter::build_context::BuildContext;
use crate::interpreter::build_context::PerFileTypeContext;

/// Errors around subrule declaration and invocation.
#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum SubruleError {
    #[error("Subrule must be assigned to a variable before use, e.g. `my_subrule = subrule(...)`")]
    SubruleNotAssigned,
    #[error("`subrule` can only be declared in .bzl files")]
    SubruleNonInBzl,
    #[error("Subrule can only be invoked after the module is loaded")]
    SubruleCalledBeforeFreezing,
}

/// The callable that's returned from a `subrule()` call. Once frozen, it can be
/// invoked from within a rule's implementation function.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct StarlarkSubruleCallable<'v> {
    /// The name of this subrule (set when exported/assigned to a variable).
    name: RefCell<Option<String>>,
    /// The implementation function for this subrule.
    implementation: Value<'v>,
    /// Attributes defined by this subrule (will be lifted into parent rules).
    attrs: Vec<(String, StarlarkAttribute)>,
    /// Configuration fragments this subrule needs access to.
    fragments: Vec<String>,
    /// Toolchains this subrule requires.
    toolchains: Vec<String>,
    /// Nested subrules this subrule depends on.
    subrules: Vec<Value<'v>>,
    /// Documentation string.
    doc: Option<String>,
}

impl<'v> Display for StarlarkSubruleCallable<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.name.borrow() {
            Some(name) => write!(f, "<subrule {}>", name),
            None => write!(f, "<unbound subrule>"),
        }
    }
}

impl<'v> AllocValue<'v> for StarlarkSubruleCallable<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl<'v> StarlarkSubruleCallable<'v> {
    fn new(
        implementation: Value<'v>,
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        fragments: Vec<String>,
        toolchains: Vec<String>,
        subrules: Vec<Value<'v>>,
        doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> kuro_error::Result<StarlarkSubruleCallable<'v>> {
        let build_context = BuildContext::from_context(eval)?;

        // Verify we're in a .bzl file
        match &build_context.additional {
            PerFileTypeContext::Bzl(_) => {}
            _ => return Err(SubruleError::SubruleNonInBzl.into()),
        }

        let attrs_vec: Vec<(String, StarlarkAttribute)> = attrs
            .entries
            .into_iter()
            .map(|(name, attr)| (name.to_owned(), StarlarkAttribute::new(attr.clone_attribute())))
            .collect();

        Ok(StarlarkSubruleCallable {
            name: RefCell::new(None),
            implementation,
            attrs: attrs_vec,
            fragments,
            toolchains,
            subrules,
            doc: if doc.is_empty() {
                None
            } else {
                Some(doc.to_owned())
            },
        })
    }
}

#[starlark_value(type = "Subrule")]
impl<'v> StarlarkValue<'v> for StarlarkSubruleCallable<'v> {
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
        // Subrules cannot be called before freezing
        Err(kuro_error::Error::from(SubruleError::SubruleCalledBeforeFreezing).into())
    }

    fn documentation(&self) -> DocItem {
        let params = ParametersSpec::<FrozenValue>::new_named_only(
            "subrule",
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
        Ty::function(ParamSpec::kwargs(Ty::any()), Ty::any())
    }
}

/// Frozen (immutable) version of StarlarkSubruleCallable.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("<subrule {}>", name)]
pub struct FrozenStarlarkSubruleCallable {
    /// The name of this subrule.
    name: String,
    /// The implementation function for this subrule.
    implementation: FrozenValue,
    /// Attributes defined by this subrule.
    attrs: Vec<(String, StarlarkAttribute)>,
    /// Configuration fragments this subrule needs access to.
    fragments: Vec<String>,
    /// Toolchains this subrule requires.
    toolchains: Vec<String>,
    /// Documentation string.
    doc: Option<String>,
}

starlark_simple_value!(FrozenStarlarkSubruleCallable);

impl<'v> Freeze for StarlarkSubruleCallable<'v> {
    type Frozen = FrozenStarlarkSubruleCallable;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let name = match self.name.into_inner() {
            Some(name) => name,
            None => {
                return Err(FreezeError::new(SubruleError::SubruleNotAssigned.to_string()));
            }
        };

        Ok(FrozenStarlarkSubruleCallable {
            name,
            implementation: self.implementation.freeze(freezer)?,
            attrs: self.attrs,
            fragments: self.fragments,
            toolchains: self.toolchains,
            doc: self.doc,
        })
    }
}

impl FrozenStarlarkSubruleCallable {
    /// Get the name of this subrule.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the attributes defined by this subrule.
    pub fn attrs(&self) -> &[(String, StarlarkAttribute)] {
        &self.attrs
    }

    /// Get the fragments this subrule requires.
    pub fn fragments(&self) -> &[String] {
        &self.fragments
    }

    /// Get the toolchains this subrule requires.
    pub fn toolchains(&self) -> &[String] {
        &self.toolchains
    }

    /// Get the implementation function.
    pub fn implementation(&self) -> FrozenValue {
        self.implementation
    }
}

#[starlark_value(type = "Subrule")]
impl<'v> StarlarkValue<'v> for FrozenStarlarkSubruleCallable {
    type Canonical = StarlarkSubruleCallable<'v>;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // When a frozen subrule is invoked from a rule's implementation,
        // we call the implementation function with the provided arguments.
        // The first argument should be the subrule context (similar to ctx).
        //
        // For now, we pass through all arguments to the implementation function.
        // In a full implementation, we would construct a proper subrule_ctx object.
        self.implementation.invoke(args, eval)
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
        StarlarkSubruleCallable::get_type_starlark_repr()
    }
}

/// Register the `subrule()` function as a Starlark global.
#[starlark_module]
pub fn register_subrule_function(builder: &mut GlobalsBuilder) {
    /// Define a subrule - a reusable building block for rule implementations.
    ///
    /// Subrules allow encapsulating shared functionality that can be used across
    /// multiple rules. When a rule declares a subrule in its `subrules` parameter,
    /// the subrule's attributes are lifted into the rule, and the subrule's
    /// implementation can be called from the rule's implementation.
    ///
    /// Example:
    /// ```python
    /// def _my_subrule_impl(subrule_ctx, *, _some_attr):
    ///     return struct(result = "computed")
    ///
    /// my_subrule = subrule(
    ///     implementation = _my_subrule_impl,
    ///     attrs = {
    ///         "_some_attr": attr.label(default = "//some:target"),
    ///     },
    ///     fragments = ["cpp"],
    /// )
    /// ```
    fn subrule<'v>(
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named, default = UnpackDictEntries::default())]
        attrs: UnpackDictEntries<&'v str, &'v StarlarkAttribute>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        fragments: UnpackListOrTuple<String>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        toolchains: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        subrules: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = "")] doc: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<StarlarkSubruleCallable<'v>> {
        // Convert toolchains to strings (they're typically labels but we store as strings for now)
        let toolchains_strings: Vec<String> = toolchains
            .items
            .iter()
            .map(|v| v.to_str())
            .collect();

        Ok(StarlarkSubruleCallable::new(
            implementation,
            attrs,
            fragments.items,
            toolchains_strings,
            subrules.items,
            doc,
            eval,
        )?)
    }

    /// Type symbol for Subrule.
    const Subrule: StarlarkValueAsType<StarlarkSubruleCallable> = StarlarkValueAsType::new();
}
