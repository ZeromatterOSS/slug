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
//! Reference: thoughts/shared/plans/kuro-bazel-subplans/03-rule-primitives.md

use allocative::Allocative;
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
use starlark::values::none::NoneType;
use starlark::values::starlark_value;
use std::fmt;
use std::fmt::Display;

// ============================================================================
// CppFragment - C++ configuration fragment
// ============================================================================

/// C++ configuration fragment.
///
/// Accessed via `ctx.fragments.cpp`. Contains C++ build settings like
/// compilation mode, PIC requirements, coverage format, etc.
///
/// Reference: rules_cc uses these in cc/private/toolchain_config/configure_features.bzl
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CppFragment {
    /// Compilation mode: "opt", "dbg", or "fastbuild"
    compilation_mode: String,
    /// Whether PIC is forced
    force_pic: bool,
    /// Whether to use LLVM coverage map format
    use_llvm_coverage_map_format: bool,
    /// Whether to generate dSYM files on macOS
    apple_generate_dsym: bool,
}

impl Default for CppFragment {
    fn default() -> Self {
        Self {
            compilation_mode: "fastbuild".to_owned(),
            force_pic: false,
            use_llvm_coverage_map_format: false,
            apple_generate_dsym: false,
        }
    }
}

impl Display for CppFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<cpp fragment>")
    }
}

starlark_simple_value!(CppFragment);

#[starlark_value(type = "cpp_fragment")]
impl<'v> StarlarkValue<'v> for CppFragment {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cpp_fragment_methods)
    }
}

#[starlark_module]
fn cpp_fragment_methods(builder: &mut MethodsBuilder) {
    /// Returns whether -fPIC is forced for compilation.
    fn force_pic(this: &CppFragment) -> starlark::Result<bool> {
        Ok(this.force_pic)
    }

    /// Returns the compilation mode: "opt", "dbg", or "fastbuild".
    fn compilation_mode(this: &CppFragment) -> starlark::Result<String> {
        Ok(this.compilation_mode.clone())
    }

    /// Returns whether to use LLVM coverage map format.
    fn use_llvm_coverage_map_format(this: &CppFragment) -> starlark::Result<bool> {
        Ok(this.use_llvm_coverage_map_format)
    }

    /// Returns the FDO instrumentation path, or None if not set.
    fn fdo_instrument(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        // TODO(fragments): Implement FDO instrumentation path
        Ok(NoneType)
    }

    /// Returns the CS-FDO instrumentation path, or None if not set.
    fn cs_fdo_instrument(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        // TODO(fragments): Implement CS-FDO instrumentation path
        Ok(NoneType)
    }

    /// Whether to generate dSYM files on macOS.
    #[starlark(attribute)]
    fn apple_generate_dsym(this: &CppFragment) -> starlark::Result<bool> {
        Ok(this.apple_generate_dsym)
    }

    /// Whether to generate linkmaps for Objective-C.
    #[starlark(attribute)]
    fn objc_generate_linkmap(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether to strip Objective-C binaries.
    #[starlark(attribute)]
    fn objc_should_strip_binary(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }
}

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
        matches!(attribute, "cpp" | "java" | "apple" | "platform")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "cpp" => Some(heap.alloc(self.cpp.clone())),
            // TODO(fragments): Implement other fragments as needed
            "java" | "apple" | "platform" => Some(heap.alloc(NoneType)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "cpp".to_owned(),
            "java".to_owned(),
            "apple".to_owned(),
            "platform".to_owned(),
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

impl CppFragment {
    /// Create a new CppFragment with the specified settings.
    pub fn new(
        compilation_mode: String,
        force_pic: bool,
        use_llvm_coverage_map_format: bool,
        apple_generate_dsym: bool,
    ) -> Self {
        Self {
            compilation_mode,
            force_pic,
            use_llvm_coverage_map_format,
            apple_generate_dsym,
        }
    }
}

impl Clone for CppFragment {
    fn clone(&self) -> Self {
        Self {
            compilation_mode: self.compilation_mode.clone(),
            force_pic: self.force_pic,
            use_llvm_coverage_map_format: self.use_llvm_coverage_map_format,
            apple_generate_dsym: self.apple_generate_dsym,
        }
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
