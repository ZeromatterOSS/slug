/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible coverage_common module.
//!
//! This provides an implementation of Bazel's coverage_common built-in module
//! for code coverage instrumentation support.
//!
//! Reference: https://bazel.build/rules/lib/toplevel/coverage_common

use allocative::Allocative;
use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::dict::Dict;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use std::fmt;
use std::fmt::Display;

// ============================================================================
// CoverageCommonModule - The main coverage_common namespace
// ============================================================================

/// The coverage_common module provides code coverage utilities.
///
/// This module is used by rulesets to configure instrumentation for code coverage.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CoverageCommonModule;

impl Display for CoverageCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "coverage_common")
    }
}

starlark_simple_value!(CoverageCommonModule);

#[starlark_value(type = "coverage_common")]
impl<'v> StarlarkValue<'v> for CoverageCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(coverage_common_module_methods)
    }
}

// ============================================================================
// InstrumentedFilesInfo - Provider for instrumented files
// ============================================================================

/// InstrumentedFilesInfo provider stub.
///
/// This provider carries information about which files are instrumented for coverage.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct InstrumentedFilesInfoInstance;

impl Display for InstrumentedFilesInfoInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InstrumentedFilesInfo()")
    }
}

starlark_simple_value!(InstrumentedFilesInfoInstance);

#[starlark_value(type = "InstrumentedFilesInfo")]
impl<'v> StarlarkValue<'v> for InstrumentedFilesInfoInstance {}

/// Methods on the coverage_common module.
#[starlark_module]
fn coverage_common_module_methods(builder: &mut MethodsBuilder) {
    /// Creates an InstrumentedFilesInfo provider.
    ///
    /// This is a stub - actual coverage support is not yet implemented.
    fn instrumented_files_info<'v>(
        #[starlark(this)] _this: &CoverageCommonModule,
        #[starlark(require = named)] _ctx: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] _source_attributes: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] _dependency_attributes: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] _extensions: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)] _metadata_files: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<InstrumentedFilesInfoInstance> {
        // TODO: Implement actual coverage instrumentation support
        Ok(InstrumentedFilesInfoInstance)
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register the coverage_common global.
#[starlark_module]
pub fn register_coverage_common(globals: &mut GlobalsBuilder) {
    /// The coverage_common module for code coverage utilities.
    const coverage_common: CoverageCommonModule = CoverageCommonModule;
}
