/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible cc_common module and CcInfo provider stubs.
//!
//! This provides a minimal implementation of Bazel's cc_common built-in
//! and CcInfo provider to allow loading rules_cc and other C/C++ rule sets.
//!
//! TODO: Implement full cc_common functionality as needed.

use allocative::Allocative;
use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::starlark_value;
use std::fmt;
use std::fmt::Display;

/// The cc_common module provides C/C++ compilation support.
///
/// This is a stub implementation for Bazel compatibility.
/// It provides the minimal interface needed to load rules_cc.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcCommonModule;

impl Display for CcCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cc_common")
    }
}

starlark_simple_value!(CcCommonModule);

#[starlark_value(type = "cc_common")]
impl<'v> StarlarkValue<'v> for CcCommonModule {
    // This is intentionally minimal - cc_common doesn't need to do much yet.
    // The key is that it exists so `hasattr(cc_common, ...)` works.
}

/// CcInfo provider stub - contains C++ compilation and linking information.
///
/// In Bazel, CcInfo is a core provider that carries:
/// - Compilation context (headers, defines, includes)
/// - Linking context (libraries, linker flags)
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcInfoProvider;

impl Display for CcInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcInfo>")
    }
}

starlark_simple_value!(CcInfoProvider);

#[starlark_value(type = "CcInfo")]
impl<'v> StarlarkValue<'v> for CcInfoProvider {
    // CcInfo is a provider type - it's used for type annotations
    // and can be called to create instances.
}

/// Register the cc_common global and CcInfo provider.
#[starlark_module]
pub fn register_cc_common(globals: &mut GlobalsBuilder) {
    /// The cc_common module provides C/C++ compilation support.
    ///
    /// This is a stub implementation for Bazel compatibility.
    const cc_common: CcCommonModule = CcCommonModule;

    /// CcInfo provider for C++ compilation and linking information.
    ///
    /// This is a stub provider for Bazel compatibility.
    const CcInfo: CcInfoProvider = CcInfoProvider;
}
