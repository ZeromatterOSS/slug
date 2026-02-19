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
use kuro_node::attrs::attr::Attribute;
use kuro_node::attrs::attr_type::AttrType;
use kuro_node::attrs::coerced_attr::CoercedAttr;
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

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum StarlarkAttributeError {
    #[error("`attrs.default_only()` cannot be used in nested attributes")]
    DefaultOnlyInNested,
}

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct StarlarkAttribute {
    inner: Attribute,
    /// True if this is an `attr.output()` attribute (Bazel output file declaration).
    pub is_output: bool,
}

impl std::fmt::Display for StarlarkAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.inner, f)
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
        }
    }

    pub fn new_output(attr: Attribute) -> Self {
        Self {
            inner: attr,
            is_output: true,
        }
    }

    pub fn clone_attribute(&self) -> Attribute {
        self.inner.clone()
    }

    /// Coercer to put into higher lever coercer (e. g. for `attrs.list(xxx)`).
    pub fn coercer_for_inner(&self) -> kuro_error::Result<AttrType> {
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
