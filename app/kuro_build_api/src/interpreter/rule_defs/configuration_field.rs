/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible configuration_field() stub.
//!
//! In Bazel, configuration_field() references values from configuration fragments.
//! This is a stub that returns None to allow rules to load.

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

/// A reference to a configuration field value.
///
/// This is a stub - actual configuration fragment support is not yet implemented.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConfigurationFieldRef {
    fragment: String,
    name: String,
}

impl Display for ConfigurationFieldRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "configuration_field({}, {})", self.fragment, self.name)
    }
}

starlark_simple_value!(ConfigurationFieldRef);

#[starlark_value(type = "configuration_field")]
impl<'v> StarlarkValue<'v> for ConfigurationFieldRef {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "fragment" | "name")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "fragment" => Some(_heap.alloc_str(&self.fragment).to_value()),
            "name" => Some(_heap.alloc_str(&self.name).to_value()),
            _ => None,
        }
    }
}

/// Register the configuration_field global.
#[starlark_module]
pub fn register_configuration_field(globals: &mut GlobalsBuilder) {
    /// References a value from a configuration fragment.
    ///
    /// Args:
    ///     fragment: The name of the configuration fragment (e.g., "cpp", "coverage").
    ///     name: The name of the field within the fragment.
    ///
    /// Returns:
    ///     A reference to the configuration value.
    fn configuration_field(
        #[starlark(require = named)] fragment: &str,
        #[starlark(require = named)] name: &str,
    ) -> starlark::Result<ConfigurationFieldRef> {
        // TODO: Implement actual configuration fragment support
        Ok(ConfigurationFieldRef {
            fragment: fragment.to_owned(),
            name: name.to_owned(),
        })
    }
}
