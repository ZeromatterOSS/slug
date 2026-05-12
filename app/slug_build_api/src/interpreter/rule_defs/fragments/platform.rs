/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Platform configuration fragment.

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
use starlark::values::starlark_value;

// ============================================================================
// PlatformFragment - Platform configuration fragment
// ============================================================================

/// Platform configuration fragment.
///
/// Accessed via `ctx.fragments.platform`. Contains platform-related settings.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PlatformFragment;

impl Display for PlatformFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<platform fragment>")
    }
}

starlark_simple_value!(PlatformFragment);

#[starlark_value(type = "platform_fragment")]
impl<'v> StarlarkValue<'v> for PlatformFragment {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(platform_fragment_methods)
    }
}

#[starlark_module]
fn platform_fragment_methods(builder: &mut MethodsBuilder) {
    /// Returns the target platform label.
    #[starlark(attribute)]
    fn platform(#[allow(unused_variables)] this: &PlatformFragment) -> starlark::Result<String> {
        Ok("@local_config_platform//:host".to_owned())
    }

    /// Returns the host platform label.
    #[starlark(attribute)]
    fn host_platform(
        #[allow(unused_variables)] this: &PlatformFragment,
    ) -> starlark::Result<String> {
        Ok("@local_config_platform//:host".to_owned())
    }
}
