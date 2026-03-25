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
use std::fmt::Debug;
use std::fmt::Display;
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
use starlark::starlark_complex_value;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Demand;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
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

    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        args.no_positional_args(heap)?;
        let kwargs = args.names_map()?;
        let mut label = String::new();
        let mut constraint_setting_label = String::new();
        for (k, v) in kwargs.iter() {
            match k.as_str() {
                "label" => label = v.to_str(),
                "constraint" | "constraint_setting" => constraint_setting_label = v.to_str(),
                _ => {}
            }
        }
        Ok(heap.alloc(ConstraintValueInfoInstance {
            constraint_setting_label,
            label,
        }))
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
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let kwargs = args.names_map()?;
        let fields = eval
            .heap()
            .alloc(AllocDict(kwargs.into_iter().map(|(k, v)| (k.as_str(), v))));
        Ok(eval
            .heap()
            .alloc(ConstraintSettingInfoInstanceGen { fields }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of ConstraintSettingInfo created by calling the provider.
///
/// Created by `platform_common.ConstraintSettingInfo(label=...)`.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    starlark::values::Trace,
    starlark::coerce::Coerce,
    starlark::values::Freeze
)]
#[repr(C)]
pub struct ConstraintSettingInfoInstanceGen<V: ValueLifetimeless> {
    fields: V,
}

starlark_complex_value!(pub ConstraintSettingInfoInstance);

impl<V: ValueLifetimeless> Display for ConstraintSettingInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ConstraintSettingInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for ConstraintSettingInfoInstanceGen<V>
where
    Self: Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        ConstraintSettingInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![]
    }
}

#[starlark::values::starlark_value(type = "ConstraintSettingInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for ConstraintSettingInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        if let Ok(iter) = self.fields.to_value().iterate(heap) {
            for key in iter {
                if key.unpack_str() == Some(attribute) {
                    return true;
                }
            }
        }
        false
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let key = heap.alloc_str(attribute);
        self.fields.to_value().at(key.to_value(), heap).ok()
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// PlatformInfo - Provider for platform information
// ============================================================================

/// PlatformInfo provider callable.
///
/// In Bazel, `platform()` targets provide PlatformInfo. It can also be created directly:
/// ```python
/// platform_common.PlatformInfo(label=..., constraints=[...])
/// ```
///
/// Used as a key for provider indexing: `target[platform_common.PlatformInfo]`
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PlatformInfoProvider;

impl PlatformInfoProvider {
    /// Get the static provider ID for PlatformInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "PlatformInfo".to_owned(),
            })
        })
    }
}

impl Display for PlatformInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider PlatformInfo>")
    }
}

starlark_simple_value!(PlatformInfoProvider);

impl ProviderCallableLike for PlatformInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "PlatformInfo")]
impl<'v> StarlarkValue<'v> for PlatformInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Collect all kwargs into a dict for the instance
        let kwargs = args.names_map()?;
        let fields = eval
            .heap()
            .alloc(AllocDict(kwargs.into_iter().map(|(k, v)| (k.as_str(), v))));
        Ok(eval.heap().alloc(PlatformInfoInstanceGen { fields }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of PlatformInfo with label and constraints fields.
///
/// Created by calling `platform_common.PlatformInfo(label=..., constraints=[...])`.
/// The fields are accessible as attributes.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    starlark::values::Trace,
    starlark::coerce::Coerce,
    starlark::values::Freeze
)]
#[repr(C)]
pub struct PlatformInfoInstanceGen<V: ValueLifetimeless> {
    /// Fields stored as a dict
    fields: V,
}

starlark_complex_value!(pub PlatformInfoInstance);

impl<V: ValueLifetimeless> Display for PlatformInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlatformInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for PlatformInfoInstanceGen<V>
where
    Self: Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        PlatformInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![]
    }
}

#[starlark::values::starlark_value(type = "PlatformInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for PlatformInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        if let Ok(iter) = self.fields.to_value().iterate(heap) {
            for key in iter {
                if key.unpack_str() == Some(attribute) {
                    return true;
                }
            }
        }
        false
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let key = heap.alloc_str(attribute);
        self.fields.to_value().at(key.to_value(), heap).ok()
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// ToolchainInfo - Provider for toolchain information
// ============================================================================

/// ToolchainInfo provider callable.
///
/// In Bazel, toolchain rule implementations return `platform_common.ToolchainInfo(...)`
/// to declare what they provide. The kwargs become attributes on the instance:
///
/// ```python
/// def _cc_toolchain_impl(ctx):
///     return [platform_common.ToolchainInfo(cc = cc_info, ...)]
/// ```
///
/// Instances are stored in provider collections and retrieved via
/// `target[platform_common.ToolchainInfo]` or `ctx.toolchains["//type"].field`.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ToolchainInfoProvider;

impl ToolchainInfoProvider {
    /// Get the static provider ID for ToolchainInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "ToolchainInfo".to_owned(),
            })
        })
    }
}

impl Display for ToolchainInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider ToolchainInfo>")
    }
}

starlark_simple_value!(ToolchainInfoProvider);

impl ProviderCallableLike for ToolchainInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "ToolchainInfo")]
impl<'v> StarlarkValue<'v> for ToolchainInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Collect all kwargs into a dict
        let kwargs = args.names_map()?;
        let fields = eval.heap().alloc(starlark::values::dict::AllocDict(
            kwargs.into_iter().map(|(k, v)| (k.as_str(), v)),
        ));
        Ok(eval.heap().alloc(ToolchainInfoInstanceGen { fields }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of ToolchainInfo with dynamic fields.
///
/// Created by calling `platform_common.ToolchainInfo(field1=val1, field2=val2, ...)`.
/// The fields are accessible as attributes: `info.field1`, `info.field2`, etc.
/// Fields are stored as a Starlark dict value to properly handle freezing.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    starlark::values::Trace,
    starlark::coerce::Coerce,
    starlark::values::Freeze
)]
#[repr(C)]
pub struct ToolchainInfoInstanceGen<V: starlark::values::ValueLifetimeless> {
    /// Fields stored as a dict
    fields: V,
}

starlark_complex_value!(pub ToolchainInfoInstance);

impl<V: starlark::values::ValueLifetimeless> Display for ToolchainInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ToolchainInfo(...)")
    }
}

impl<'v, V: starlark::values::ValueLike<'v>> ProviderLike<'v> for ToolchainInfoInstanceGen<V>
where
    Self: Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        ToolchainInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        // Items are exposed via get_attr
        vec![]
    }
}

#[starlark::values::starlark_value(type = "ToolchainInfo")]
impl<'v, V: starlark::values::ValueLike<'v>> StarlarkValue<'v> for ToolchainInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        if let Ok(iter) = self.fields.to_value().iterate(heap) {
            for key in iter {
                if key.unpack_str() == Some(attribute) {
                    return true;
                }
            }
        }
        false
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        let key = heap.alloc_str(attribute);
        self.fields.to_value().at(key.to_value(), heap).ok()
    }

    fn dir_attr(&self) -> Vec<String> {
        if let Some(dict) = DictRef::from_value(self.fields.to_value()) {
            return dict
                .keys()
                .filter_map(|k| k.unpack_str().map(|s| s.to_owned()))
                .collect();
        }
        vec![]
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// TemplateVariableInfo - Provider for Make variable values
// ============================================================================

/// Callable for creating TemplateVariableInfo instances.
/// Used as platform_common.TemplateVariableInfo({...}).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct TemplateVariableInfoCallable;

impl TemplateVariableInfoCallable {
    /// Get the static provider ID for TemplateVariableInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "TemplateVariableInfo".to_owned(),
            })
        })
    }
}

impl Display for TemplateVariableInfoCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider TemplateVariableInfo>")
    }
}

starlark_simple_value!(TemplateVariableInfoCallable);

impl ProviderCallableLike for TemplateVariableInfoCallable {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

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

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
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

impl<'v> ProviderLike<'v> for TemplateVariableInfoInstance {
    fn id(&self) -> &Arc<ProviderId> {
        TemplateVariableInfoCallable::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![]
    }
}

#[starlark_value(type = "TemplateVariableInfo")]
impl<'v> StarlarkValue<'v> for TemplateVariableInfoInstance {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(template_variable_info_instance_methods)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
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
