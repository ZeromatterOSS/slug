/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Implementation of Bazel's `macro()` built-in function for symbolic macros.
//!
//! Symbolic macros (Bazel 8.0+) are a first-class macro system that provides
//! better introspection and encapsulation than legacy function-based macros.
//!
//! ## Current Implementation
//!
//! This is a functional implementation that:
//! - Creates a callable `MacroCallable` from `macro(implementation=fn, attrs={...})`
//! - When called in a BUILD file, invokes the implementation function with the
//!   provided arguments (`name`, `visibility`, and declared attrs)
//! - Stores attribute declarations for validation
//!
//! ## Not Yet Implemented
//!
//! - Name enforcement (targets created inside must start with macro's `name`)
//! - Visibility encapsulation (targets inside are private by default)
//! - `select()` wrapping of configurable attributes
//! - `inherit_attrs` inheritance from rules/macros
//! - `finalizer` deferred execution
//! - Macro call stack tracking
//!
//! See: https://bazel.build/rules/lib/globals/bzl#macro

use std::cell::RefCell;
use std::fmt;

use allocative::Allocative;
use derive_more::Display;
use slug_node::attrs::coerced_attr::CoercedAttr;
use starlark::any::ProvidesStaticType;
use starlark::collections::SmallMap;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
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
use starlark::values::dict::DictRef;
use starlark::values::list::AllocList;
use starlark::values::starlark_value;

use crate::attrs::starlark_attribute::StarlarkAttribute;
use crate::rule::FrozenStarlarkRuleCallable;

/// Errors around macro declaration and invocation.
#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum MacroError {
    #[error("Macro must be assigned to a variable before use, e.g. `my_macro = macro(...)`")]
    MacroNotAssigned,
    #[error("Macro can only be invoked after the module is loaded")]
    MacroCalledBeforeFreezing,
}

/// The callable that's returned from a `macro()` call. Once frozen, it can be
/// invoked in BUILD files to create targets.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct StarlarkMacroCallable<'v> {
    /// The name of this macro (set when exported/assigned to a variable).
    name: RefCell<Option<String>>,
    /// The implementation function for this macro.
    implementation: Value<'v>,
    /// Whether this is a finalizer macro (runs after all non-finalizer targets).
    finalizer: bool,
    /// Documentation string.
    doc: Option<String>,
    /// The `attrs` dict from `macro(attrs={...})`. Used to apply defaults for
    /// parameters not provided by the caller.
    attrs: Option<Value<'v>>,
    /// Rule or macro whose public attributes are inherited by this macro.
    inherit_attrs: Option<Value<'v>>,
}

impl<'v> fmt::Display for StarlarkMacroCallable<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.name.borrow() {
            Some(name) => write!(f, "<macro {}>", name),
            None => write!(f, "<unbound macro>"),
        }
    }
}

impl<'v> AllocValue<'v> for StarlarkMacroCallable<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

#[starlark_value(type = "macro")]
impl<'v> StarlarkValue<'v> for StarlarkMacroCallable<'v> {
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
        // Macros cannot be called before freezing (they're defined in .bzl, called in BUILD)
        Err(slug_error::Error::from(MacroError::MacroCalledBeforeFreezing).into())
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::function(ParamSpec::kwargs(Ty::any()), Ty::any())
    }
}

/// Frozen (immutable) version of StarlarkMacroCallable.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
#[display("<macro {}>", name)]
pub struct FrozenStarlarkMacroCallable {
    /// The name of this macro.
    name: String,
    /// The implementation function for this macro.
    implementation: FrozenValue,
    /// Whether this is a finalizer macro.
    finalizer: bool,
    /// Documentation string.
    doc: Option<String>,
    /// The `attrs` dict from `macro(attrs={...})`. Used to apply defaults.
    attrs: Option<FrozenValue>,
    /// Frozen rule or macro whose public attributes are inherited by this macro.
    inherit_attrs: Option<FrozenValue>,
}

starlark::starlark_simple_value!(FrozenStarlarkMacroCallable);

impl<'v> Freeze for StarlarkMacroCallable<'v> {
    type Frozen = FrozenStarlarkMacroCallable;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let name = match self.name.into_inner() {
            Some(name) => name,
            None => {
                return Err(FreezeError::new(MacroError::MacroNotAssigned.to_string()));
            }
        };

        Ok(FrozenStarlarkMacroCallable {
            name,
            implementation: self.implementation.freeze(freezer)?,
            finalizer: self.finalizer,
            doc: self.doc,
            attrs: self.attrs.freeze(freezer)?,
            inherit_attrs: self.inherit_attrs.freeze(freezer)?,
        })
    }
}

#[starlark_value(type = "macro")]
impl<'v> StarlarkValue<'v> for FrozenStarlarkMacroCallable {
    type Canonical = StarlarkMacroCallable<'v>;

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // When a frozen macro is invoked from a BUILD file, call the
        // implementation function with the provided arguments.
        //
        // In Bazel, the implementation function receives:
        //   def _impl(name, visibility, attr1, attr2, ..., **kwargs):
        //
        // Bazel's macro framework automatically injects `visibility` if not
        // provided by the caller, and applies defaults from attrs={...} for
        // any declared attributes not provided by the caller.
        let heap = eval.heap();
        let names_map = args.names_map()?;
        let positional: Vec<Value<'v>> = args.positions(heap)?.collect();
        let mut named: Vec<(&str, Value<'v>)> = Vec::new();
        for (k, v) in names_map.iter() {
            named.push((k.as_str(), *v));
        }
        if !named.iter().any(|(name, _)| *name == "visibility") {
            named.push(("visibility", Value::new_none()));
        }

        if let Some(attrs_val) = &self.attrs {
            if let Some(dict) = DictRef::from_value(attrs_val.to_value()) {
                for (k, v) in dict.iter() {
                    let Some(attr_name) = k.unpack_str() else {
                        continue;
                    };
                    if named.iter().any(|(name, _)| *name == attr_name) {
                        continue;
                    }
                    if let Some(sa) = v.downcast_ref::<StarlarkAttribute>() {
                        let default_val = if sa.implicit_default {
                            Value::new_none()
                        } else {
                            sa.default().map_or_else(Value::new_none, |default| {
                                coerced_attr_default_to_value(default.as_ref(), heap)
                            })
                        };
                        let name_ref = heap.alloc_str(attr_name);
                        named.push((name_ref.as_str(), default_val));
                    }
                }
            }
        }

        if let Some(inherit_attrs) = &self.inherit_attrs {
            if let Some(rule) = inherit_attrs
                .to_value()
                .downcast_ref::<FrozenStarlarkRuleCallable>()
            {
                for (attr_name, _attr_id, attr) in rule.attributes().attr_specs() {
                    if attr_name == "name" || attr_name == "visibility" {
                        continue;
                    }
                    if named.iter().any(|(name, _)| *name == attr_name) {
                        continue;
                    }
                    let default_val = if rule.has_implicit_default_attr(attr_name) {
                        Value::new_none()
                    } else {
                        attr.default().map_or_else(Value::new_none, |default| {
                            coerced_attr_default_to_value(default.as_ref(), heap)
                        })
                    };
                    let name_ref = heap.alloc_str(attr_name);
                    named.push((name_ref.as_str(), default_val));
                }
            }
        }

        eval.eval_function(self.implementation.to_value(), &positional, &named)
            .map_err(Into::into)
    }

    fn get_type_starlark_repr() -> Ty {
        StarlarkMacroCallable::get_type_starlark_repr()
    }
}

impl FrozenStarlarkMacroCallable {
    /// Get the name of this macro.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether this is a finalizer macro.
    pub fn is_finalizer(&self) -> bool {
        self.finalizer
    }
}

impl<'v> StarlarkMacroCallable<'v> {
    pub fn new(
        implementation: Value<'v>,
        finalizer: bool,
        doc: Option<String>,
        attrs: Option<Value<'v>>,
        inherit_attrs: Option<Value<'v>>,
    ) -> StarlarkMacroCallable<'v> {
        StarlarkMacroCallable {
            name: RefCell::new(None),
            implementation,
            finalizer,
            doc,
            attrs,
            inherit_attrs,
        }
    }
}

/// Convert a CoercedAttr default to a Starlark Value for injection into macro calls.
/// Handles common default types; complex cases (labels, deps) fall back to None.
fn coerced_attr_default_to_value<'v>(default: &CoercedAttr, heap: Heap<'v>) -> Value<'v> {
    match default {
        CoercedAttr::None => Value::new_none(),
        CoercedAttr::Bool(b) => heap.alloc(b.0),
        CoercedAttr::Int(i) => heap.alloc(*i),
        CoercedAttr::String(s) | CoercedAttr::EnumVariant(s) => heap.alloc(s.as_str()),
        CoercedAttr::List(list) => {
            let items: Vec<Value<'v>> = list
                .iter()
                .map(|v| coerced_attr_default_to_value(v, heap))
                .collect();
            heap.alloc(AllocList(items))
        }
        CoercedAttr::Dict(map) => {
            let mut res = SmallMap::with_capacity(map.len());
            for pair in map.iter() {
                let kv = coerced_attr_default_to_value(&pair.0, heap);
                let vv = coerced_attr_default_to_value(&pair.1, heap);
                if let Ok(hashed) = kv.get_hashed() {
                    res.insert_hashed(hashed, vv);
                }
            }
            heap.alloc(starlark::values::dict::Dict::new(res))
        }
        // For labels, deps, and other complex types, fall back to None
        _ => Value::new_none(),
    }
}
