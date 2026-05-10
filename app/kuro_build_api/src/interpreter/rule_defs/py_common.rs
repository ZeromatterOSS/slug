/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible PyInfo and PyRuntimeInfo providers.
//!
//! These are native Starlark globals in Bazel. rules_python 1.8.0+
//! references them as `BuiltinPyInfo = PyInfo` in reexports.bzl.
//!
//! With `enable_pystar = True`, rules_python creates both its own Starlark
//! PyInfo AND the BuiltinPyInfo (this native provider) for backward compat.

use std::fmt;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use kuro_core::provider::id::ProviderId;
use kuro_interpreter::types::provider::callable::ProviderCallableLike;
use starlark::coerce::Coerce;
use starlark::environment::GlobalsBuilder;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::dict::AllocDict;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::provider::ProviderLike;

// ============================================================================
// NativeProviderInstance - Generic instance for callable native providers
// ============================================================================

/// A provider instance created by calling a native provider callable like
/// PyInfo or PyRuntimeInfo. Stores keyword arguments as a dict value and
/// exposes them as attributes.
///
/// Follows the same pattern as OutputGroupInfoInstanceGen in cc_common.rs.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct NativeProviderInstanceGen<V: ValueLifetimeless> {
    /// All keyword arguments stored as a dict value
    pub values: V,
    /// Provider ID index: 0 = PyInfo, 1 = PyRuntimeInfo, 2 = JavaInfo, 3 = JavaPluginInfo
    /// (Using u32 instead of Arc<ProviderId> to keep the type Coerce/Freeze-friendly)
    pub provider_idx: u32,
}

starlark_complex_value!(pub NativeProviderInstance);

pub fn provider_id_for_idx(idx: u32) -> &'static Arc<ProviderId> {
    match idx {
        0 => PyInfoProvider::provider_id(),
        1 => PyRuntimeInfoProvider::provider_id(),
        2 => crate::interpreter::rule_defs::java_common::JavaInfoProvider::provider_id(),
        3 => crate::interpreter::rule_defs::java_common::JavaPluginInfoProvider::provider_id(),
        4 => crate::interpreter::rule_defs::java_common::JavaRuntimeInfoProvider::provider_id(),
        5 => crate::interpreter::rule_defs::java_common::JavaToolchainInfoProvider::provider_id(),
        6 => crate::interpreter::rule_defs::proto_common::ProtoInfoProvider::provider_id(),
        7 => {
            crate::interpreter::rule_defs::proto_common::ProtoLangToolchainInfoProvider::provider_id(
            )
        }
        8 => AnalysisTestResultInfoProvider::provider_id(),
        _ => panic!("Invalid provider index: {}", idx),
    }
}

pub fn provider_name_for_idx(idx: u32) -> &'static str {
    match idx {
        0 => "PyInfo",
        1 => "PyRuntimeInfo",
        2 => "JavaInfo",
        3 => "JavaPluginInfo",
        4 => "JavaRuntimeInfo",
        5 => "JavaToolchainInfo",
        6 => "ProtoInfo",
        7 => "ProtoLangToolchainInfo",
        8 => "AnalysisTestResultInfo",
        _ => "Unknown",
    }
}

impl<V: ValueLifetimeless> Display for NativeProviderInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(...)", provider_name_for_idx(self.provider_idx))
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for NativeProviderInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        provider_id_for_idx(self.provider_idx)
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.values.to_value()) {
            dict.iter()
                .filter_map(|(k, v)| {
                    let s: &'v str = k.unpack_str()?;
                    // SAFETY: 'v outlives &self since self contains V: ValueLike<'v>,
                    // so the string data is valid for the duration of the &self borrow.
                    let s: &str = unsafe { &*(s as *const str) };
                    Some((s, v))
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

#[starlark_value(type = "struct")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for NativeProviderInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.values.to_value()) {
            dict.get_str(attribute).is_some()
        } else if let Ok(iter) = self.values.to_value().iterate(heap) {
            for key in iter {
                if key.unpack_str() == Some(attribute) {
                    return true;
                }
            }
            false
        } else {
            false
        }
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.values.to_value()) {
            dict.get_str(attribute).map(|v| v)
        } else {
            let key = heap.alloc_str(attribute);
            self.values.to_value().at(key.to_value(), heap).ok()
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.values.to_value()) {
            dict.iter()
                .filter_map(|(k, _)| k.unpack_str().map(|s| s.to_owned()))
                .collect()
        } else {
            Vec::new()
        }
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// PyInfo - Provider for Python information
// ============================================================================

/// PyInfo provider callable.
///
/// Called as `PyInfo(transitive_sources=..., imports=..., ...)` to create instances.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PyInfoProvider;

impl PyInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "PyInfo".to_owned(),
            })
        })
    }
}

impl Display for PyInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider PyInfo>")
    }
}

starlark_simple_value!(PyInfoProvider);

impl ProviderCallableLike for PyInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "PyInfo")]
impl<'v> StarlarkValue<'v> for PyInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        create_native_provider_instance(0, args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// PyRuntimeInfo - Provider for Python runtime information
// ============================================================================

/// PyRuntimeInfo provider callable.
///
/// Called as `PyRuntimeInfo(interpreter=..., files=..., ...)` to create instances.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PyRuntimeInfoProvider;

impl PyRuntimeInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "PyRuntimeInfo".to_owned(),
            })
        })
    }
}

impl Display for PyRuntimeInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider PyRuntimeInfo>")
    }
}

starlark_simple_value!(PyRuntimeInfoProvider);

impl ProviderCallableLike for PyRuntimeInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "PyRuntimeInfo")]
impl<'v> StarlarkValue<'v> for PyRuntimeInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        create_native_provider_instance(1, args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// AnalysisTestResultInfo - Provider for analysis test results
// ============================================================================

#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AnalysisTestResultInfoProvider;

impl AnalysisTestResultInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "AnalysisTestResultInfo".to_owned(),
            })
        })
    }
}

impl Display for AnalysisTestResultInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider AnalysisTestResultInfo>")
    }
}

starlark_simple_value!(AnalysisTestResultInfoProvider);

impl ProviderCallableLike for AnalysisTestResultInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "AnalysisTestResultInfo")]
impl<'v> StarlarkValue<'v> for AnalysisTestResultInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        create_native_provider_instance(8, args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// Shared invoke helper
// ============================================================================

/// Creates a native provider instance from keyword arguments.
pub fn create_native_provider_instance<'v>(
    provider_idx: u32,
    args: &Arguments<'v, '_>,
    eval: &mut Evaluator<'v, '_, '_>,
) -> starlark::Result<Value<'v>> {
    let heap = eval.heap();
    args.no_positional_args(heap)?;

    // Collect kwargs into a dict
    let kwargs = args.names_map()?;
    let pairs: Vec<(Value<'v>, Value<'v>)> =
        kwargs.into_iter().map(|(k, v)| (k.to_value(), v)).collect();
    let dict = heap.alloc(AllocDict(pairs));

    Ok(heap.alloc(NativeProviderInstance {
        values: dict,
        provider_idx,
    }))
}

// ============================================================================
// Registration
// ============================================================================

/// Register PyInfo and PyRuntimeInfo as Starlark globals.
#[starlark_module]
pub fn register_py_common(globals: &mut GlobalsBuilder) {
    /// PyInfo provider for Python source and import information.
    const PyInfo: PyInfoProvider = PyInfoProvider;

    /// PyRuntimeInfo provider for Python runtime/interpreter information.
    const PyRuntimeInfo: PyRuntimeInfoProvider = PyRuntimeInfoProvider;
}
