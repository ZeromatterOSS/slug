/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible platform_common module.
//!
//! This provides an implementation of Bazel's platform_common built-in module
//! that rules_cc and other rulesets require for platform configuration.
//!
//! The platform_common module provides:
//! - `TemplateVariableInfo` - Provider for Make variable values
//! - Platform-related utilities
//!
//! Reference: https://bazel.build/rules/lib/toplevel/platform_common

use std::fmt;
use std::fmt::Display;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use kuro_core::provider::id::ProviderId;
use kuro_interpreter::types::provider::callable::ProviderCallableLike;
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
use starlark::values::dict::DictRef;
use starlark::values::starlark_value;
use starlark_map::small_map::SmallMap;

use crate::interpreter::rule_defs::provider::ProviderLike;

// ============================================================================
// PlatformCommonModule - The main platform_common namespace
// ============================================================================

/// The platform_common module provides platform and Make variable utilities.
///
/// This module is used by rulesets like rules_cc to provide Make variables
/// to dependent targets.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PlatformCommonModule;

impl Display for PlatformCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "platform_common")
    }
}

starlark_simple_value!(PlatformCommonModule);

#[starlark_value(type = "platform_common")]
impl<'v> StarlarkValue<'v> for PlatformCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(platform_common_module_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "TemplateVariableInfo"
                | "ToolchainInfo"
                | "ConstraintValueInfo"
                | "ConstraintSettingInfo"
                | "PlatformInfo"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "TemplateVariableInfo" => Some(heap.alloc(TemplateVariableInfoCallable)),
            "ToolchainInfo" => Some(heap.alloc(ToolchainInfoProvider)),
            "ConstraintValueInfo" => Some(heap.alloc(ConstraintValueInfoProvider)),
            "ConstraintSettingInfo" => Some(heap.alloc(ConstraintSettingInfoProvider)),
            "PlatformInfo" => Some(heap.alloc(PlatformInfoProvider)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "TemplateVariableInfo".to_owned(),
            "ToolchainInfo".to_owned(),
            "ConstraintValueInfo".to_owned(),
            "ConstraintSettingInfo".to_owned(),
            "PlatformInfo".to_owned(),
        ]
    }
}

// ============================================================================
// ConstraintValueInfo - Provider for constraint values
// ============================================================================

/// ConstraintValueInfo provider type.
///
/// In Bazel, constraint_value targets provide this. It contains:
/// - constraint: The ConstraintSettingInfo of the constraint setting
/// - label: The label of the constraint value
///
/// Used as a key for provider indexing: `target[platform_common.ConstraintValueInfo]`
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConstraintValueInfoProvider;

impl ConstraintValueInfoProvider {
    /// Get the static provider ID for ConstraintValueInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "ConstraintValueInfo".to_owned(),
            })
        })
    }
}

impl Display for ConstraintValueInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider ConstraintValueInfo>")
    }
}

starlark_simple_value!(ConstraintValueInfoProvider);

impl ProviderCallableLike for ConstraintValueInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "ConstraintValueInfo")]
impl<'v> StarlarkValue<'v> for ConstraintValueInfoProvider {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(constraint_value_info_provider_methods)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of ConstraintValueInfo (simple/frozen version).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConstraintValueInfoInstance {
    pub constraint_setting_label: String,
    pub label: String,
}

impl Display for ConstraintValueInfoInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConstraintValueInfo(label=\"{}\")", self.label)
    }
}

starlark_simple_value!(ConstraintValueInfoInstance);

impl<'v> ProviderLike<'v> for ConstraintValueInfoInstance {
    fn id(&self) -> &Arc<ProviderId> {
        ConstraintValueInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        // Items are returned via get_attr; this is for provider collection
        vec![]
    }
}

#[starlark_value(type = "ConstraintValueInfo")]
impl<'v> StarlarkValue<'v> for ConstraintValueInfoInstance {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "constraint" | "label")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "constraint" => Some(heap.alloc_str(&self.constraint_setting_label).to_value()),
            "label" => Some(heap.alloc_str(&self.label).to_value()),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec!["constraint".to_owned(), "label".to_owned()]
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

/// Methods on ConstraintValueInfo provider callable.
#[starlark_module]
fn constraint_value_info_provider_methods(builder: &mut MethodsBuilder) {
    // No methods needed for now - just a provider type marker
}

// ============================================================================
// ConstraintSettingInfo - Provider for constraint settings
// ============================================================================

/// ConstraintSettingInfo provider type.
///
/// In Bazel, constraint_setting targets provide this. It contains:
/// - label: The label of the constraint setting
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConstraintSettingInfoProvider;

impl ConstraintSettingInfoProvider {
    /// Get the static provider ID for ConstraintSettingInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "ConstraintSettingInfo".to_owned(),
            })
        })
    }
}

impl Display for ConstraintSettingInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider ConstraintSettingInfo>")
    }
}

starlark_simple_value!(ConstraintSettingInfoProvider);

impl ProviderCallableLike for ConstraintSettingInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "ConstraintSettingInfo")]
impl<'v> StarlarkValue<'v> for ConstraintSettingInfoProvider {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// PlatformInfo - Provider for platform information
// ============================================================================

/// PlatformInfo provider type.
///
/// TODO(bazel): Implement full platform info semantics.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PlatformInfoProvider;

impl Display for PlatformInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider PlatformInfo>")
    }
}

starlark_simple_value!(PlatformInfoProvider);

#[starlark_value(type = "PlatformInfo")]
impl<'v> StarlarkValue<'v> for PlatformInfoProvider {}

// ============================================================================
// ToolchainInfo - Provider for toolchain information
// ============================================================================

/// ToolchainInfo provider for toolchain resolution.
///
/// This provider is used by toolchain rules to declare toolchain capabilities.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ToolchainInfoProvider;

impl Display for ToolchainInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider ToolchainInfo>")
    }
}

starlark_simple_value!(ToolchainInfoProvider);

#[starlark_value(type = "ToolchainInfo")]
impl<'v> StarlarkValue<'v> for ToolchainInfoProvider {}

// ============================================================================
// TemplateVariableInfo - Provider for Make variable values
// ============================================================================

/// Callable for creating TemplateVariableInfo instances.
/// Used as platform_common.TemplateVariableInfo({...}).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct TemplateVariableInfoCallable;

impl Display for TemplateVariableInfoCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider TemplateVariableInfo>")
    }
}

starlark_simple_value!(TemplateVariableInfoCallable);

#[starlark_value(type = "TemplateVariableInfo")]
impl<'v> StarlarkValue<'v> for TemplateVariableInfoCallable {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TemplateVariableInfo(variables_dict) - creates a provider instance
        // Parse positional argument manually
        let args_iter = args.positions(eval.heap())?;
        let variables = args_iter.into_iter().next().ok_or_else(|| {
            starlark::Error::new_value(starlark::values::ValueError::IncorrectParameterTypeNamed(
                "variables".to_owned(),
            ))
        })?;

        // Convert the variables dict to our internal representation
        let dict = DictRef::from_value(variables).ok_or_else(|| {
            starlark::Error::new_value(starlark::values::ValueError::IncorrectParameterTypeNamed(
                "variables".to_owned(),
            ))
        })?;

        // Store the variables as String -> String mapping
        let mut template_vars: SmallMap<String, String> = SmallMap::new();
        for (k, v) in dict.iter() {
            let key = k.unpack_str().ok_or_else(|| {
                starlark::Error::new_value(
                    starlark::values::ValueError::IncorrectParameterTypeNamed(
                        "variables key".to_owned(),
                    ),
                )
            })?;
            let value = v.unpack_str().ok_or_else(|| {
                starlark::Error::new_value(
                    starlark::values::ValueError::IncorrectParameterTypeNamed(
                        "variables value".to_owned(),
                    ),
                )
            })?;
            template_vars.insert(key.to_owned(), value.to_owned());
        }

        Ok(eval.heap().alloc(TemplateVariableInfoInstance {
            variables: template_vars,
        }))
    }
}

/// An instance of TemplateVariableInfo with the actual variable values.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct TemplateVariableInfoInstance {
    variables: SmallMap<String, String>,
}

impl Display for TemplateVariableInfoInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TemplateVariableInfo({:?})", self.variables)
    }
}

starlark_simple_value!(TemplateVariableInfoInstance);

#[starlark_value(type = "TemplateVariableInfo")]
impl<'v> StarlarkValue<'v> for TemplateVariableInfoInstance {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(template_variable_info_instance_methods)
    }
}

/// Methods/attributes on a TemplateVariableInfo instance.
#[starlark_module]
fn template_variable_info_instance_methods(builder: &mut MethodsBuilder) {
    /// The variables dictionary (returns the internal SmallMap for now).
    /// In a full implementation, this would allocate a dict on the heap.
    fn get_variables<'v>(
        this: &TemplateVariableInfoInstance,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Convert the SmallMap<String, String> to a dict on the heap
        use starlark::values::dict::AllocDict;
        let items: Vec<(&str, &str)> = this
            .variables
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        Ok(eval.heap().alloc(AllocDict(items)))
    }
}

/// Methods on the platform_common module.
#[starlark_module]
fn platform_common_module_methods(builder: &mut MethodsBuilder) {
    // TemplateVariableInfo is accessed as an attribute, not a method
    // See get_attr implementation above
}

// ============================================================================
// Registration
// ============================================================================

/// Register the platform_common global.
#[starlark_module]
pub fn register_platform_common(globals: &mut GlobalsBuilder) {
    /// The platform_common module for platform and Make variable utilities.
    const platform_common: PlatformCommonModule = PlatformCommonModule;
}
