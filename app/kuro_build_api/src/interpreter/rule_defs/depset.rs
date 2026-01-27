/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible depset implementation.
//!
//! In Bazel, a depset is an immutable collection that supports efficient
//! union operations. This is similar to Kuro's transitive_set but with
//! a different API.
//!
//! This is currently a stub implementation that allows rules_cc to load.
//! TODO: Implement full depset functionality.

use allocative::Allocative;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list_or_tuple::UnpackListOrTuple;
use starlark::values::starlark_value;
use std::fmt;
use std::fmt::Display;

/// A Bazel-compatible depset (directed acyclic graph of elements).
///
/// This is currently a stub that stores elements directly.
/// TODO: Implement proper transitive/deduplication behavior.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct Depset {
    // For now, just store whether it's empty or not
    // A full implementation would store the actual elements and children
    is_empty: bool,
}

impl Display for Depset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "depset([])")
    }
}

starlark_simple_value!(Depset);

#[starlark_value(type = "depset")]
impl<'v> StarlarkValue<'v> for Depset {
    fn to_bool(&self) -> bool {
        !self.is_empty
    }

    fn length(&self) -> starlark::Result<i32> {
        // TODO: Implement actual length
        Ok(0)
    }
}

/// Register the depset global function.
#[starlark_module]
pub fn register_depset(globals: &mut GlobalsBuilder) {
    /// Create a depset (immutable set with efficient union).
    ///
    /// Args:
    ///     direct: Elements to include directly in this depset.
    ///     transitive: Other depsets whose elements should be transitively included.
    ///     order: Iteration order ("default", "preorder", "postorder", "topological").
    ///
    /// Returns:
    ///     A new depset containing the specified elements.
    fn depset<'v>(
        #[starlark(default = UnpackListOrTuple::default())] direct: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = UnpackListOrTuple::default())]
        transitive: UnpackListOrTuple<Value<'v>>,
        #[starlark(require = named, default = "default")] order: &str,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Depset> {
        // TODO: Implement proper depset behavior
        let _unused = (order,);
        let is_empty = direct.items.is_empty() && transitive.items.is_empty();
        Ok(Depset { is_empty })
    }
}
