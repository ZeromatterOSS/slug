/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible config_common module.
//!
//! This provides an implementation of Bazel's config_common built-in module
//! that rules_cc and other rulesets require for toolchain configuration.
//!
//! The config_common module provides:
//! - `toolchain_type()` - Creates a toolchain type requirement
//! - Configuration-related utilities
//!
//! Reference: https://bazel.build/rules/lib/config_common

use allocative::Allocative;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::dict::Dict;
use starlark::values::none::NoneOr;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;
use std::fmt;
use std::fmt::Display;

// ============================================================================
// ConfigCommonModule - The main config_common namespace
// ============================================================================

/// The config_common module provides configuration and toolchain utilities.
///
/// This module is used by rulesets like rules_cc to specify toolchain requirements
/// for rules.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConfigCommonModule;

impl Display for ConfigCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "config_common")
    }
}

starlark_simple_value!(ConfigCommonModule);

#[starlark_value(type = "config_common")]
impl<'v> StarlarkValue<'v> for ConfigCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(config_common_module_methods)
    }
}

// ============================================================================
// ToolchainTypeRequirement - Result of toolchain_type()
// ============================================================================

/// A toolchain type requirement returned by config_common.toolchain_type().
///
/// This struct represents a dependency on a specific toolchain type
/// that a rule requires.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct ToolchainTypeRequirement {
    /// The label of the toolchain type (e.g., "@bazel_tools//tools/cpp:toolchain_type")
    toolchain_type: String,
    /// Whether this toolchain is mandatory (true) or optional (false)
    mandatory: bool,
}

impl Display for ToolchainTypeRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ToolchainTypeRequirement(toolchain_type = \"{}\", mandatory = {})",
            self.toolchain_type, self.mandatory
        )
    }
}

starlark_simple_value!(ToolchainTypeRequirement);

#[starlark_value(type = "ToolchainTypeRequirement")]
impl<'v> StarlarkValue<'v> for ToolchainTypeRequirement {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(toolchain_type_requirement_methods)
    }
}

/// Methods/attributes on a ToolchainTypeRequirement.
#[starlark_module]
fn toolchain_type_requirement_methods(builder: &mut MethodsBuilder) {
    /// The toolchain type label.
    #[starlark(attribute)]
    fn toolchain_type(this: &ToolchainTypeRequirement) -> starlark::Result<String> {
        Ok(this.toolchain_type.clone())
    }

    /// Whether this toolchain requirement is mandatory.
    #[starlark(attribute)]
    fn mandatory(this: &ToolchainTypeRequirement) -> starlark::Result<bool> {
        Ok(this.mandatory)
    }
}

/// Methods on the config_common module.
#[starlark_module]
fn config_common_module_methods(builder: &mut MethodsBuilder) {
    /// Creates a toolchain type requirement.
    ///
    /// This is used in rule definitions to specify that a rule needs a
    /// particular toolchain (e.g., C++ toolchain).
    ///
    /// Args:
    ///   toolchain_type: The label of the toolchain type
    ///   mandatory: Whether the toolchain is required (default: True)
    ///
    /// Returns:
    ///   A ToolchainTypeRequirement that can be used in rule definitions.
    fn toolchain_type<'v>(
        #[starlark(this)] _this: &ConfigCommonModule,
        #[starlark(require = pos)] toolchain_type: &str,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ToolchainTypeRequirement> {
        Ok(ToolchainTypeRequirement {
            toolchain_type: toolchain_type.to_owned(),
            mandatory: mandatory.unwrap_or(true),
        })
    }

    /// Checks if feature flags are enabled.
    ///
    /// This is a stub that always returns false for now.
    fn feature_flag_info<'v>(
        #[starlark(this)] _this: &ConfigCommonModule,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return None/empty - feature flags not implemented
        Ok(Value::new_none())
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register the config_common global.
#[starlark_module]
pub fn register_config_common(globals: &mut GlobalsBuilder) {
    /// The config_common module for configuration and toolchain utilities.
    const config_common: ConfigCommonModule = ConfigCommonModule;
}
