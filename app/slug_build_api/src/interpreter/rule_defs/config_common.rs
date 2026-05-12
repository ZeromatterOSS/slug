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
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use slug_core::provider::id::ProviderId;
use slug_interpreter::types::provider::callable::ProviderCallableLike;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Demand;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::provider::ProviderLike;

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
            "FeatureFlagInfo" => Some(heap.alloc(FeatureFlagInfoProvider)),
            _ => None,
        }
    }
}

// ============================================================================
// FeatureFlagInfo - Provider for feature flags (config_common.FeatureFlagInfo)
// ============================================================================

/// The callable provider type for config_common.FeatureFlagInfo.
/// When called as `config_common.FeatureFlagInfo(value = ...)`, creates a
/// FeatureFlagInfoInstance.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FeatureFlagInfoProvider;

impl FeatureFlagInfoProvider {
    /// Get the static provider ID for FeatureFlagInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "FeatureFlagInfo".to_owned(),
            })
        })
    }
}

impl Display for FeatureFlagInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider FeatureFlagInfo>")
    }
}

starlark_simple_value!(FeatureFlagInfoProvider);

impl ProviderCallableLike for FeatureFlagInfoProvider {
    fn id(&self) -> slug_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "FeatureFlagInfo")]
impl<'v> StarlarkValue<'v> for FeatureFlagInfoProvider {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let kwargs = args.names_map()?;
        // Extract the `value` kwarg and convert it to a string representation.
        // FeatureFlagInfo is primarily used for string/bool/int build settings.
        let value_str = kwargs
            .iter()
            .find(|(k, _)| k.as_str() == "value")
            .map(|(_, v)| {
                if let Some(s) = v.unpack_str() {
                    s.to_owned()
                } else {
                    format!("{v}")
                }
            })
            .unwrap_or_default();
        Ok(heap.alloc(FeatureFlagInfoInstance { value: value_str }))
    }
}

/// A FeatureFlagInfo provider instance created by `config_common.FeatureFlagInfo(value = ...)`.
///
/// This provider is produced by feature flag rules (like `string_flag`, `bool_flag`)
/// and consumed by `config_setting` targets using `flag_values`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FeatureFlagInfoInstance {
    value: String,
}

impl Display for FeatureFlagInfoInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FeatureFlagInfo(value=\"{}\")", self.value)
    }
}

starlark_simple_value!(FeatureFlagInfoInstance);

impl<'v> ProviderLike<'v> for FeatureFlagInfoInstance {
    fn id(&self) -> &Arc<ProviderId> {
        FeatureFlagInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        // Items() needs Value<'v> but we only store String; return empty for now.
        // Field access is handled via get_attr() which has a heap available.
        vec![]
    }
}

#[starlark_value(type = "FeatureFlagInfo")]
impl<'v> StarlarkValue<'v> for FeatureFlagInfoInstance {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "value" => Some(heap.alloc(self.value.clone())),
            _ => None,
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        attribute == "value"
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
