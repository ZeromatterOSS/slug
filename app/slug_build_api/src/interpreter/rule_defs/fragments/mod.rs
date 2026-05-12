/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Configuration fragments for Bazel compatibility.
//!
//! In Bazel, `ctx.fragments` provides access to configuration fragments like
//! `ctx.fragments.cpp`, `ctx.fragments.java`, etc. These fragments contain
//! build configuration settings.
//!
//! Reference: thoughts/shared/plans/slug-bazel-subplans/03-rule-primitives.md

mod apple;
mod coverage;
mod cpp;
mod java;
mod platform;
mod proto;
mod py;

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
pub use apple::AppleFragment;
pub use apple::ApplePlatformStub;
pub use coverage::CoverageFragment;
pub use cpp::CppFragment;
pub use java::JavaFragment;
pub use platform::PlatformFragment;
pub use proto::ProtoFragment;
pub use py::BazelPyFragment;
pub use py::PyFragment;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

// ============================================================================
// ConfigurationFragments - Container for all fragments
// ============================================================================

/// Container for configuration fragments.
///
/// Accessed via `ctx.fragments`. Provides access to language-specific
/// configuration fragments like `cpp`, `java`, `apple`, etc.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConfigurationFragments {
    cpp: CppFragment,
}

impl Default for ConfigurationFragments {
    fn default() -> Self {
        Self {
            cpp: CppFragment::default(),
        }
    }
}

impl Display for ConfigurationFragments {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx.fragments>")
    }
}

starlark_simple_value!(ConfigurationFragments);

#[starlark_value(type = "configuration_fragments")]
impl<'v> StarlarkValue<'v> for ConfigurationFragments {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(configuration_fragments_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "cpp" | "py" | "bazel_py" | "java" | "apple" | "platform" | "proto" | "coverage"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "cpp" => Some(heap.alloc(self.cpp.clone())),
            "py" => Some(heap.alloc(PyFragment)),
            "bazel_py" => Some(heap.alloc(BazelPyFragment)),
            "proto" => Some(heap.alloc(ProtoFragment)),
            "java" => Some(heap.alloc(JavaFragment)),
            "apple" => Some(heap.alloc(AppleFragment)),
            "platform" => Some(heap.alloc(PlatformFragment)),
            "coverage" => Some(heap.alloc(CoverageFragment)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "cpp".to_owned(),
            "py".to_owned(),
            "bazel_py".to_owned(),
            "java".to_owned(),
            "apple".to_owned(),
            "platform".to_owned(),
            "proto".to_owned(),
            "coverage".to_owned(),
        ]
    }
}

#[starlark_module]
fn configuration_fragments_methods(builder: &mut MethodsBuilder) {
    /// C++ configuration fragment.
    #[starlark(attribute)]
    fn cpp<'v>(this: &ConfigurationFragments, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(this.cpp.clone()))
    }
}

impl ConfigurationFragments {
    /// Create new configuration fragments with the given cpp fragment.
    pub fn new(cpp: CppFragment) -> Self {
        Self { cpp }
    }
}

impl Clone for ConfigurationFragments {
    fn clone(&self) -> Self {
        Self {
            cpp: self.cpp.clone(),
        }
    }
}
