/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use slug_node::attrs::attr::Attribute;
use slug_node::attrs::attr_type::AttrType;
use slug_node::attrs::coerced_attr::CoercedAttr;
use starlark::any::ProvidesStaticType;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::starlark_value;
use starlark::values::starlark_value_as_type::StarlarkValueAsType;

#[derive(Debug, slug_error::Error)]
#[slug(tag = Input)]
enum StarlarkAttributeError {
    #[error("`attrs.default_only()` cannot be used in nested attributes")]
    DefaultOnlyInNested,
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkAttribute {
    inner: Attribute,
    /// True if this is an `attr.output()` attribute (Bazel output file declaration).
    pub is_output: bool,
    /// True if the default was auto-injected by Bazel compatibility code (not user-provided).
    /// When true, the default is omitted from the repr (e.g., `attrs.string()` not
    /// `attrs.string(default="")`).
    pub implicit_default: bool,
}

impl std::fmt::Display for StarlarkAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.implicit_default {
            // Auto-injected default: omit it from repr to match Bazel behavior
            self.inner.coercer().fmt_with_default(f, None)
        } else {
            std::fmt::Display::fmt(&self.inner, f)
        }
    }
}

starlark_simple_value!(StarlarkAttribute);

/// Type of the attribute object returned by methods under [`attrs`](../attrs) namespace, e. g. `attrs.string()`.
#[starlark_module]
fn starlark_attribute_methods(builder: &mut MethodsBuilder) {}

#[starlark_value(type = "Attr")]
impl<'v> StarlarkValue<'v> for StarlarkAttribute {
    // Used to add type documentation to the generated documentation
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(starlark_attribute_methods)
    }
}

impl StarlarkAttribute {
    pub fn new(attr: Attribute) -> Self {
        Self {
            inner: attr,
            is_output: false,
            implicit_default: false,
        }
    }

    pub fn new_output(attr: Attribute) -> Self {
        Self {
            inner: attr,
            is_output: true,
            implicit_default: false,
        }
    }

    /// Create an attribute whose default was auto-injected by Bazel compatibility (not user-provided).
    /// The default is omitted from `repr()` to match Bazel behavior.
    pub fn new_with_implicit_default(attr: Attribute) -> Self {
        Self {
            inner: attr,
            is_output: false,
            implicit_default: true,
        }
    }

    pub fn clone_attribute(&self) -> Attribute {
        self.inner.clone()
    }

    /// Coercer to put into higher lever coercer (e. g. for `attrs.list(xxx)`).
    pub fn coercer_for_inner(&self) -> slug_error::Result<AttrType> {
        if self.inner.is_default_only() {
            return Err(StarlarkAttributeError::DefaultOnlyInNested.into());
        }
        Ok(self.inner.coercer().dupe())
    }

    pub fn coercer_for_default_only(&self) -> AttrType {
        self.inner.coercer().dupe()
    }

    pub fn default(&self) -> Option<&Arc<CoercedAttr>> {
        self.inner.default()
    }

    /// Get configuration_field info if this attr's default was a configuration_field().
    pub fn configuration_field(&self) -> Option<(&str, &str)> {
        self.inner.configuration_field()
    }
}

#[starlark_module]
pub(crate) fn register_attr_type(globals: &mut GlobalsBuilder) {
    /// Starlark type of the attribute object (for example, returned from `attrs.string()`).
    const Attr: StarlarkValueAsType<StarlarkAttribute> = StarlarkValueAsType::new();
}
