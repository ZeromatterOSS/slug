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

use allocative::Allocative;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::dict::DictRef;
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
        matches!(attribute, "TemplateVariableInfo" | "ToolchainInfo")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "TemplateVariableInfo" => Some(heap.alloc(TemplateVariableInfoCallable)),
            "ToolchainInfo" => Some(heap.alloc(ToolchainInfoProvider)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "TemplateVariableInfo".to_owned(),
            "ToolchainInfo".to_owned(),
        ]
    }
}

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
                starlark::Error::new_value(starlark::values::ValueError::IncorrectParameterTypeNamed(
                    "variables key".to_owned(),
                ))
            })?;
            let value = v.unpack_str().ok_or_else(|| {
                starlark::Error::new_value(starlark::values::ValueError::IncorrectParameterTypeNamed(
                    "variables value".to_owned(),
                ))
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
