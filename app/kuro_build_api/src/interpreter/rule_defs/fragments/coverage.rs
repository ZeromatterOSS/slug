/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Coverage configuration fragment.

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

// ============================================================================
// CoverageFragment - Coverage configuration fragment
// ============================================================================

/// Coverage configuration fragment.
///
/// Accessed via `ctx.fragments.coverage`. Contains coverage-related build settings.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CoverageFragment;

impl Display for CoverageFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<coverage fragment>")
    }
}

starlark_simple_value!(CoverageFragment);

#[starlark_value(type = "coverage_fragment")]
impl<'v> StarlarkValue<'v> for CoverageFragment {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(coverage_fragment_methods)
    }
}

#[starlark_module]
fn coverage_fragment_methods(builder: &mut MethodsBuilder) {
    /// The list of file extensions for which coverage output is generated.
    #[starlark(attribute)]
    fn output_generator(
        #[allow(unused_variables)] this: &CoverageFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }
}
