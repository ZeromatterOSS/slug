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

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::Dict;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;

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

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "FeatureFlagInfo")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            // Stub provider callable for feature flags - returns a callable
            // that creates struct-like objects with the given kwargs.
            "FeatureFlagInfo" => Some(heap.alloc(FeatureFlagInfoProvider)),
            _ => None,
        }
    }
}

// ============================================================================
// FeatureFlagInfoProvider - Stub provider for config_common.FeatureFlagInfo
// ============================================================================

/// A stub callable provider for config_common.FeatureFlagInfo.
/// When called, returns a struct with the provided fields (e.g., value).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FeatureFlagInfoProvider;

impl Display for FeatureFlagInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FeatureFlagInfo")
    }
}

starlark_simple_value!(FeatureFlagInfoProvider);

#[starlark_value(type = "FeatureFlagInfo")]
impl<'v> StarlarkValue<'v> for FeatureFlagInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Create a simple struct from the kwargs.
        // FeatureFlagInfo(value = "foo") -> struct(value = "foo")
        let heap = eval.heap();
        let kwargs = args.names_map()?;
        let mut entries = SmallMap::new();
        for (name, value) in kwargs.iter_hashed() {
            let key: Value<'v> = heap.alloc(name.key().as_str());
            entries.insert_hashed(key.get_hashed()?, *value);
        }
        Ok(heap.alloc(Dict::new(entries)))
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
        #[starlark(require = pos)] toolchain_type: Value<'v>,
        #[starlark(require = named)] mandatory: Option<bool>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<ToolchainTypeRequirement> {
        // Accept both plain strings and Label() objects (which return BazelLabel type).
        // Label objects display as the full resolved label string.
        // Accept both plain strings and Label() objects (BazelLabel type).
        // Label objects display as the full resolved label string.
        let toolchain_type_str = if let Some(s) = toolchain_type.unpack_str() {
            s.to_owned()
        } else {
            // For Label objects and other display types, use Display which returns full label
            format!("{}", toolchain_type)
        };
        Ok(ToolchainTypeRequirement {
            toolchain_type: toolchain_type_str,
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
