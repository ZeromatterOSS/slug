/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Pure provider / value-type definitions for cc_common.
//!
//! Every Starlark provider Kuro exposes to rules_cc lives here: compilation /
//! linking data types (CcCompilationContext, CcToolchainVariables,
//! CompilationOutputs, LibraryToLink, CcLinkingOutputs, LinkerInputStub,
//! LinkingContextWithInputs, CcDebugContext, HeaderInfoStub) and provider
//! callables + instances (CcInfo, CcToolchainInfo, CcToolchainConfigInfo,
//! DebugPackageInfo, CcSharedLibraryInfo, CcSharedLibraryHintInfo,
//! OutputGroupInfo, ExecutionInfo, RunEnvironmentInfo,
//! PackageSpecificationInfo).

use std::fmt;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use kuro_core::provider::id::ProviderId;
use kuro_interpreter::types::provider::callable::ProviderCallableLike;
use starlark::coerce::Coerce;
use starlark::collections::SmallMap;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocList;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::provider::ProviderLike;
// ============================================================================
// CcCompilationContext - Compilation context for C++ builds
// ============================================================================

/// CcCompilationContext holds the compilation context (headers, includes, defines).
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
pub struct CcCompilationContextGen<V: ValueLifetimeless> {
    /// Headers depset
    pub(crate) headers: V,
    /// Include directories (generic -I)
    pub(crate) includes: V,
    /// Quote include directories (-iquote)
    pub(crate) quote_includes: V,
    /// System include directories (-isystem)
    pub(crate) system_includes: V,
    /// External include directories. rules_cc's
    /// `init_cc_compilation_context` writes the `-I` paths for
    /// external-repo cc_libraries here (instead of `includes`) when the
    /// label's repo is non-empty. Stored separately because the
    /// downstream compile-action wiring iterates this field with `-I`
    /// just like `includes` — collapsing it onto `system_includes`
    /// loses the path entirely whenever both are non-empty.
    pub(crate) external_includes: V,
    /// Framework include directories (-F)
    pub(crate) framework_includes: V,
    /// Defines
    pub(crate) defines: V,
    /// Local defines (not propagated to dependents)
    pub(crate) local_defines: V,
}

starlark_complex_value!(pub CcCompilationContext);

impl<V: ValueLifetimeless> Display for CcCompilationContextGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<CcCompilationContext>")
    }
}

#[starlark::values::starlark_value(type = "CcCompilationContext")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcCompilationContextGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "headers"
                | "includes"
                | "quote_includes"
                | "system_includes"
                | "framework_includes"
                | "external_includes"
                | "defines"
                | "local_defines"
                | "direct_headers"
                | "direct_public_headers"
                | "direct_private_headers"
                | "direct_textual_headers"
                | "validation_artifacts"
                | "_header_info"
                | "_exporting_module_map_files"
                | "module_map"
                | "purpose"
                | "loose_hdrs_dirs"
                | "virtual_to_original_headers"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "headers" => {
                if self.headers.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.headers.to_value())
                }
            }
            "includes" => {
                if self.includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.includes.to_value())
                }
            }
            "quote_includes" => {
                if self.quote_includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.quote_includes.to_value())
                }
            }
            "system_includes" => {
                if self.system_includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.system_includes.to_value())
                }
            }
            "external_includes" => {
                if self.external_includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.external_includes.to_value())
                }
            }
            "framework_includes" => {
                if self.framework_includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.framework_includes.to_value())
                }
            }
            "defines" => {
                if self.defines.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.defines.to_value())
                }
            }
            "local_defines" => {
                if self.local_defines.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.local_defines.to_value())
                }
            }
            "direct_headers"
            | "direct_public_headers"
            | "direct_private_headers"
            | "direct_textual_headers" => Some(heap.alloc(AllocList::EMPTY)),
            "validation_artifacts" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_header_info" => Some(heap.alloc(HeaderInfoStub)),
            "_exporting_module_map_files"
            | "module_map"
            | "loose_hdrs_dirs"
            | "virtual_to_original_headers" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "purpose" => Some(Value::new_none()),
            _ => None,
        }
    }
}

// ============================================================================
// CcToolchainVariables - Variables for C++ toolchain configuration
// ============================================================================

/// CcToolchainVariables holds build variables for C++ toolchain configuration.
///
/// Used by cc_common functions to pass configuration to compile/link actions.
/// This version stores a reference to the original variables dict for access
/// by get_link_args.
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
pub struct CcToolchainVariablesGen<V: ValueLifetimeless> {
    /// The original variables dict
    pub(crate) vars: V,
}

starlark_complex_value!(pub CcToolchainVariables);

impl<V: ValueLifetimeless> Display for CcToolchainVariablesGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcToolchainVariables()")
    }
}

#[starlark::values::starlark_value(type = "CcToolchainVariables")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcToolchainVariablesGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        // Allow attribute access to underlying variables dict
        let vars_value = self.vars.to_value();
        if vars_value.is_none() {
            return None;
        }
        // Use DictRef to access dict values by string key
        if let Some(dict_ref) = DictRef::from_value(vars_value) {
            dict_ref.get_str(attribute)
        } else {
            None
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        let vars_value = self.vars.to_value();
        if vars_value.is_none() {
            return false;
        }
        if let Some(dict_ref) = DictRef::from_value(vars_value) {
            dict_ref.get_str(attribute).is_some()
        } else {
            false
        }
    }
}
// ============================================================================
// HeaderInfoStub - Stub for header info returned by create_header_info
// ============================================================================

/// A stub for HeaderInfo returned by create_header_info.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct HeaderInfoStub;

impl Display for HeaderInfoStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<HeaderInfo>")
    }
}

starlark_simple_value!(HeaderInfoStub);

#[starlark_value(type = "HeaderInfo")]
impl<'v> StarlarkValue<'v> for HeaderInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "modular_public_headers"
                | "modular_private_headers"
                | "textual_headers"
                | "separate_module_headers"
                | "header_module"
                | "pic_header_module"
                | "separate_module"
                | "separate_pic_module"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "modular_public_headers"
            | "modular_private_headers"
            | "textual_headers"
            | "separate_module_headers" => {
                // Return empty list
                Some(heap.alloc(AllocList::EMPTY))
            }
            "header_module" | "pic_header_module" | "separate_module" | "separate_pic_module" => {
                // Return None
                Some(Value::new_none())
            }
            _ => None,
        }
    }
}

// ============================================================================
// CompilationOutputs - Outputs from C++ compilation
// ============================================================================

/// CompilationOutputs holds the output files from C++ compilation.
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
pub struct CompilationOutputsGen<V: ValueLifetimeless> {
    pub(crate) objects: V,
    pub(crate) pic_objects: V,
}

starlark_complex_value!(pub CompilationOutputs);

impl<V: ValueLifetimeless> Display for CompilationOutputsGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<CompilationOutputs>")
    }
}

/// Methods on CompilationOutputs for accessing coverage files.
#[starlark_module]
fn compilation_outputs_methods(builder: &mut MethodsBuilder) {
    /// Returns coverage (gcno) files from non-PIC compilation.
    fn gcno_files<'v>(this: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return empty list - no coverage files in this stub
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Returns coverage (gcno) files from PIC compilation.
    fn pic_gcno_files<'v>(this: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return empty list - no coverage files in this stub
        Ok(heap.alloc(AllocList::EMPTY))
    }
}

#[starlark::values::starlark_value(type = "CompilationOutputs")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CompilationOutputsGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "objects" | "pic_objects")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "objects" => Some(self.objects.to_value()),
            "pic_objects" => Some(self.pic_objects.to_value()),
            // These are additional attributes that rules_cc may access
            "_gcno_files" | "_pic_gcno_files" => Some(heap.alloc(AllocList::EMPTY)),
            _ => None,
        }
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(compilation_outputs_methods)
    }
}

// ============================================================================
// LibraryToLink - A library artifact for linking
// ============================================================================

/// LibraryToLink represents a library that can be linked.
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
pub struct LibraryToLinkGen<V: ValueLifetimeless> {
    pub(crate) static_library: V,
    pub(crate) pic_static_library: V,
    pub(crate) dynamic_library: V,
    pub(crate) interface_library: V,
    pub(crate) objects: V,
    pub(crate) pic_objects: V,
    pub(crate) alwayslink: bool,
}

starlark_complex_value!(pub LibraryToLink);

impl<V: ValueLifetimeless> Display for LibraryToLinkGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<LibraryToLink>")
    }
}

#[starlark::values::starlark_value(type = "LibraryToLink")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LibraryToLinkGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "static_library"
                | "pic_static_library"
                | "dynamic_library"
                | "interface_library"
                | "objects"
                | "pic_objects"
                | "alwayslink"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "static_library" => Some(self.static_library.to_value()),
            "pic_static_library" => Some(self.pic_static_library.to_value()),
            "dynamic_library" => Some(self.dynamic_library.to_value()),
            "interface_library" => Some(self.interface_library.to_value()),
            "objects" => {
                if self.objects.to_value().is_none() {
                    Some(heap.alloc(starlark::values::list::AllocList::EMPTY))
                } else {
                    Some(self.objects.to_value())
                }
            }
            "pic_objects" => {
                if self.pic_objects.to_value().is_none() {
                    Some(heap.alloc(starlark::values::list::AllocList::EMPTY))
                } else {
                    Some(self.pic_objects.to_value())
                }
            }
            "alwayslink" => Some(Value::new_bool(self.alwayslink)),
            _ => None,
        }
    }
}

// ============================================================================
// CcLinkingOutputs - Outputs from linking
// ============================================================================

/// CcLinkingOutputs holds the output files from C++ linking.
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
pub struct CcLinkingOutputsGen<V: ValueLifetimeless> {
    pub(crate) library_to_link: V,
    pub(crate) executable: V,
}

starlark_complex_value!(pub CcLinkingOutputs);

impl<V: ValueLifetimeless> Display for CcLinkingOutputsGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<CcLinkingOutputs>")
    }
}

#[starlark::values::starlark_value(type = "CcLinkingOutputs")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcLinkingOutputsGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "library_to_link" | "executable")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "library_to_link" => Some(self.library_to_link.to_value()),
            "executable" => Some(self.executable.to_value()),
            _ => None,
        }
    }
}

// ============================================================================
// CcToolchainInfoProvider - Provider for C++ toolchain information
// ============================================================================

/// CcToolchainInfo provider for C++ toolchain information.
///
/// This provider carries toolchain configuration like compiler paths,
/// flags, and supported features.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcToolchainInfoProvider;

impl CcToolchainInfoProvider {
    /// Get the static provider ID for CcToolchainInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "CcToolchainInfo".to_owned(),
            })
        })
    }
}

impl Display for CcToolchainInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcToolchainInfo>")
    }
}

starlark_simple_value!(CcToolchainInfoProvider);

impl ProviderCallableLike for CcToolchainInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "CcToolchainInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainInfoProvider {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// CcInfo provider - C++ compilation/linking information
// ============================================================================

/// CcInfo provider callable - contains C++ compilation and linking information.
///
/// In Bazel 9.0+, CcInfo is actually defined in pure Starlark in rules_cc
/// (cc/private/cc_info.bzl). This native stub exists for compatibility with
/// code that references the native CcInfo before rules_cc is loaded.
///
/// Implements ProviderCallableLike so it can be used as `CcInfo in dep` and
/// `dep[CcInfo]` for provider collection lookups.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcInfoProvider;

impl CcInfoProvider {
    /// Get the static provider ID for CcInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "CcInfo".to_owned(),
            })
        })
    }
}

impl Display for CcInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcInfo>")
    }
}

starlark_simple_value!(CcInfoProvider);

impl ProviderCallableLike for CcInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "CcInfo")]
impl<'v> StarlarkValue<'v> for CcInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // CcInfo(compilation_context=..., linking_context=...)
        let kwargs = args.names_map()?;
        let heap = eval.heap();
        let compilation_context = kwargs
            .get("compilation_context")
            .copied()
            .unwrap_or(Value::new_none());
        let linking_context = kwargs
            .get("linking_context")
            .copied()
            .unwrap_or(Value::new_none());
        Ok(heap.alloc(CcInfoInstanceGen {
            compilation_context,
            linking_context,
        }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// A CcInfo instance with actual compilation and linking context data.
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
pub struct CcInfoInstanceGen<V: ValueLifetimeless> {
    pub(crate) compilation_context: V,
    pub(crate) linking_context: V,
}

starlark_complex_value!(pub CcInfoInstance);

impl<V: ValueLifetimeless> Display for CcInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for CcInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn id(&self) -> &Arc<ProviderId> {
        CcInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![
            ("compilation_context", self.compilation_context.to_value()),
            ("linking_context", self.linking_context.to_value()),
        ]
    }
}

#[starlark::values::starlark_value(type = "CcInfoInstance")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "compilation_context"
                | "linking_context"
                | "_legacy_transitive_native_libraries"
                | "_debug_context"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "compilation_context" => {
                if self.compilation_context.to_value().is_none() {
                    use crate::interpreter::rule_defs::context::EmptyCompilationContext;
                    Some(heap.alloc(EmptyCompilationContext))
                } else {
                    Some(self.compilation_context.to_value())
                }
            }
            "linking_context" => {
                if self.linking_context.to_value().is_none() {
                    use crate::interpreter::rule_defs::context::EmptyLinkingContext;
                    Some(heap.alloc(EmptyLinkingContext))
                } else {
                    Some(self.linking_context.to_value())
                }
            }
            "_legacy_transitive_native_libraries" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_debug_context" => Some(heap.alloc(CcDebugContext)),
            _ => None,
        }
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

/// A stub CcInfo instance (returned when CcInfo(...) is called with no data).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcInfoInstanceStub;

impl Display for CcInfoInstanceStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcInfo(...)")
    }
}

starlark_simple_value!(CcInfoInstanceStub);

impl<'v> ProviderLike<'v> for CcInfoInstanceStub {
    fn id(&self) -> &Arc<ProviderId> {
        CcInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        Vec::new()
    }
}

#[starlark_value(type = "CcInfoInstance")]
impl<'v> StarlarkValue<'v> for CcInfoInstanceStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_info_instance_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "compilation_context"
                | "linking_context"
                | "_legacy_transitive_native_libraries"
                | "_debug_context"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use crate::interpreter::rule_defs::context::EmptyCompilationContext;
        use crate::interpreter::rule_defs::context::EmptyLinkingContext;
        match attribute {
            "compilation_context" => Some(heap.alloc(EmptyCompilationContext)),
            "linking_context" => Some(heap.alloc(EmptyLinkingContext)),
            "_legacy_transitive_native_libraries" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_debug_context" => Some(heap.alloc(CcDebugContext)),
            _ => None,
        }
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

#[starlark_module]
fn cc_info_instance_methods(builder: &mut MethodsBuilder) {
    /// Returns transitive native libraries as a depset.
    fn transitive_native_libraries<'v>(
        this: &CcInfoInstanceStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Returns the debug context for this CcInfo.
    fn debug_context<'v>(this: &CcInfoInstanceStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(CcDebugContext))
    }
}

// ============================================================================
// CcDebugContext - Debug context stub
// ============================================================================

/// Stub for Bazel's CcDebugContext, returned by cc_common.create_debug_context()
/// and cc_common.merge_debug_context().
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcDebugContext;

impl Display for CcDebugContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcDebugContext()")
    }
}

starlark_simple_value!(CcDebugContext);

#[starlark_value(type = "CcDebugContext")]
impl<'v> StarlarkValue<'v> for CcDebugContext {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "files" | "pic_files")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "files" | "pic_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            _ => None,
        }
    }
}

// ============================================================================
// DebugPackageInfo - Debug information provider
// ============================================================================

/// DebugPackageInfo provider for debug/symbol information.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct DebugPackageInfoProvider;

impl DebugPackageInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "DebugPackageInfo".to_owned(),
            })
        })
    }
}

impl Display for DebugPackageInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider DebugPackageInfo>")
    }
}

starlark_simple_value!(DebugPackageInfoProvider);

impl ProviderCallableLike for DebugPackageInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "DebugPackageInfo")]
impl<'v> StarlarkValue<'v> for DebugPackageInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let kwargs = args.names_map()?;
        let fields = heap.alloc(starlark::values::dict::AllocDict(
            kwargs.into_iter().map(|(k, v)| (k.as_str(), v)),
        ));
        Ok(heap.alloc(DebugPackageInfoInstanceGen { fields }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of DebugPackageInfo.
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
pub struct DebugPackageInfoInstanceGen<V: ValueLifetimeless> {
    fields: V,
}

starlark_complex_value!(pub DebugPackageInfoInstance);

impl<V: ValueLifetimeless> Display for DebugPackageInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DebugPackageInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for DebugPackageInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        DebugPackageInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.fields.to_value()) {
            dict.iter()
                .filter_map(|(k, v)| {
                    let s: &'v str = k.unpack_str()?;
                    let s: &str = unsafe { &*(s as *const str) };
                    Some((s, v))
                })
                .collect()
        } else {
            vec![]
        }
    }
}

#[starlark::values::starlark_value(type = "DebugPackageInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for DebugPackageInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::dict::DictRef;
        DictRef::from_value(self.fields.to_value()).and_then(|dict| dict.get_str(attribute))
    }

    fn dir_attr(&self) -> Vec<String> {
        use starlark::values::dict::DictRef;
        DictRef::from_value(self.fields.to_value())
            .map(|dict| {
                dict.iter()
                    .filter_map(|(k, _)| k.unpack_str().map(|s| s.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// CcSharedLibraryInfo - Shared library information provider
// ============================================================================

/// CcSharedLibraryInfo provider for shared library information.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcSharedLibraryInfoProvider;

impl CcSharedLibraryInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "CcSharedLibraryInfo".to_owned(),
            })
        })
    }
}

impl Display for CcSharedLibraryInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcSharedLibraryInfo>")
    }
}

starlark_simple_value!(CcSharedLibraryInfoProvider);

impl ProviderCallableLike for CcSharedLibraryInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "CcSharedLibraryInfo")]
impl<'v> StarlarkValue<'v> for CcSharedLibraryInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let kwargs = args.names_map()?;
        let fields = heap.alloc(starlark::values::dict::AllocDict(
            kwargs.into_iter().map(|(k, v)| (k.as_str(), v)),
        ));
        Ok(heap.alloc(CcSharedLibraryInfoInstanceGen { fields }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of CcSharedLibraryInfo.
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
pub struct CcSharedLibraryInfoInstanceGen<V: ValueLifetimeless> {
    fields: V,
}

starlark_complex_value!(pub CcSharedLibraryInfoInstance);

impl<V: ValueLifetimeless> Display for CcSharedLibraryInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcSharedLibraryInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for CcSharedLibraryInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        CcSharedLibraryInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.fields.to_value()) {
            dict.iter()
                .filter_map(|(k, v)| {
                    let s: &'v str = k.unpack_str()?;
                    let s: &str = unsafe { &*(s as *const str) };
                    Some((s, v))
                })
                .collect()
        } else {
            vec![]
        }
    }
}

#[starlark::values::starlark_value(type = "CcSharedLibraryInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcSharedLibraryInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::dict::DictRef;
        DictRef::from_value(self.fields.to_value()).and_then(|dict| dict.get_str(attribute))
    }

    fn dir_attr(&self) -> Vec<String> {
        use starlark::values::dict::DictRef;
        DictRef::from_value(self.fields.to_value())
            .map(|dict| {
                dict.iter()
                    .filter_map(|(k, _)| k.unpack_str().map(|s| s.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// CcSharedLibraryHintInfo - Shared library hint provider (Bazel 7.0+)
// ============================================================================

/// CcSharedLibraryHintInfo provider for shared library dependency hints.
///
/// In Bazel, this provider carries hints about shared library dependencies.
/// Available as a top-level global since Bazel 7.0.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcSharedLibraryHintInfoProvider;

impl CcSharedLibraryHintInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "CcSharedLibraryHintInfo".to_owned(),
            })
        })
    }
}

impl Display for CcSharedLibraryHintInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcSharedLibraryHintInfo>")
    }
}

starlark_simple_value!(CcSharedLibraryHintInfoProvider);

impl ProviderCallableLike for CcSharedLibraryHintInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "CcSharedLibraryHintInfo")]
impl<'v> StarlarkValue<'v> for CcSharedLibraryHintInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let kwargs = args.names_map()?;
        let fields = heap.alloc(starlark::values::dict::AllocDict(
            kwargs.into_iter().map(|(k, v)| (k.as_str(), v)),
        ));
        Ok(heap.alloc(CcSharedLibraryHintInfoInstanceGen { fields }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of CcSharedLibraryHintInfo.
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
pub struct CcSharedLibraryHintInfoInstanceGen<V: ValueLifetimeless> {
    fields: V,
}

starlark_complex_value!(pub CcSharedLibraryHintInfoInstance);

impl<V: ValueLifetimeless> Display for CcSharedLibraryHintInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcSharedLibraryHintInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for CcSharedLibraryHintInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        CcSharedLibraryHintInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.fields.to_value()) {
            dict.iter()
                .filter_map(|(k, v)| {
                    let s: &'v str = k.unpack_str()?;
                    let s: &str = unsafe { &*(s as *const str) };
                    Some((s, v))
                })
                .collect()
        } else {
            vec![]
        }
    }
}

#[starlark::values::starlark_value(type = "CcSharedLibraryHintInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcSharedLibraryHintInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        use starlark::values::dict::DictRef;
        DictRef::from_value(self.fields.to_value()).and_then(|dict| dict.get_str(attribute))
    }

    fn dir_attr(&self) -> Vec<String> {
        use starlark::values::dict::DictRef;
        DictRef::from_value(self.fields.to_value())
            .map(|dict| {
                dict.iter()
                    .filter_map(|(k, _)| k.unpack_str().map(|s| s.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// CcToolchainConfigInfo - Toolchain configuration provider
// ============================================================================

/// CcToolchainConfigInfo provider for C++ toolchain configuration.
///
/// This provider carries the full toolchain configuration including
/// compiler paths, feature flags, and action configs. Created by
/// cc_common.create_cc_toolchain_config_info().
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcToolchainConfigInfoProvider;

// ============================================================================
// OutputGroupInfo - Bazel output groups provider
// ============================================================================

/// OutputGroupInfo provider for grouping outputs.
///
/// This provider is used by rules to specify different groups of outputs
/// for different purposes (e.g., IDE support, coverage, etc.).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct OutputGroupInfoProvider;

impl OutputGroupInfoProvider {
    /// Get the static provider ID for OutputGroupInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "OutputGroupInfo".to_owned(),
            })
        })
    }
}

impl Display for OutputGroupInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider OutputGroupInfo>")
    }
}

starlark_simple_value!(OutputGroupInfoProvider);

impl ProviderCallableLike for OutputGroupInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "OutputGroupInfo")]
impl<'v> StarlarkValue<'v> for OutputGroupInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        // Get kwargs from arguments
        let kwargs = args.names_map()?;
        // Create a dict from the kwargs using AllocDict
        let groups = heap.alloc(starlark::values::dict::AllocDict(
            kwargs.into_iter().map(|(k, v)| (k.as_str(), v)),
        ));
        Ok(heap.alloc(OutputGroupInfoInstanceGen { groups }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of OutputGroupInfo containing output groups.
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
pub struct OutputGroupInfoInstanceGen<V: ValueLifetimeless> {
    /// The groups as a dict value
    groups: V,
}

starlark_complex_value!(pub OutputGroupInfoInstance);

impl<V: ValueLifetimeless> Display for OutputGroupInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OutputGroupInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for OutputGroupInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        OutputGroupInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.groups.to_value()) {
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
            vec![]
        }
    }
}

#[starlark::values::starlark_value(type = "OutputGroupInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for OutputGroupInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.groups.to_value()) {
            dict.get_str(attribute).is_some()
        } else if let Ok(iter) = self.groups.to_value().iterate(heap) {
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
        if let Some(dict) = DictRef::from_value(self.groups.to_value()) {
            dict.get_str(attribute).map(|v| v)
        } else {
            let key = heap.alloc_str(attribute);
            self.groups.to_value().at(key.to_value(), heap).ok()
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.groups.to_value()) {
            dict.iter()
                .filter_map(|(k, _)| k.unpack_str().map(|s| s.to_owned()))
                .collect()
        } else {
            Vec::new()
        }
    }

    // Support 'in' operator: `"key" in output_group_info`
    // Delegates to the underlying groups dict.
    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        // self.groups.to_value().is_in(other) checks "is other in groups dict"
        self.groups.to_value().is_in(other)
    }

    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Index into groups dict
        self.groups.to_value().at(index, heap)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

impl CcToolchainConfigInfoProvider {
    /// Get the static provider ID for CcToolchainConfigInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "CcToolchainConfigInfo".to_owned(),
            })
        })
    }
}

impl Display for CcToolchainConfigInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcToolchainConfigInfo>")
    }
}

starlark_simple_value!(CcToolchainConfigInfoProvider);

impl ProviderCallableLike for CcToolchainConfigInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "CcToolchainConfigInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainConfigInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let kwargs = args.names_map()?;
        let fields = heap.alloc(starlark::values::dict::AllocDict(
            kwargs.into_iter().map(|(k, v)| (k.as_str(), v)),
        ));
        Ok(heap.alloc(CcToolchainConfigInfoInstanceGen { fields }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of CcToolchainConfigInfo containing toolchain configuration.
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
pub struct CcToolchainConfigInfoInstanceGen<V: ValueLifetimeless> {
    /// The fields as a dict value
    pub(crate) fields: V,
}

starlark_complex_value!(pub CcToolchainConfigInfoInstance);

impl<V: ValueLifetimeless> Display for CcToolchainConfigInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcToolchainConfigInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for CcToolchainConfigInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        CcToolchainConfigInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.fields.to_value()) {
            dict.iter()
                .filter_map(|(k, v)| {
                    let s: &'v str = k.unpack_str()?;
                    let s: &str = unsafe { &*(s as *const str) };
                    Some((s, v))
                })
                .collect()
        } else {
            vec![]
        }
    }
}

#[starlark::values::starlark_value(type = "CcToolchainConfigInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcToolchainConfigInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        use starlark::values::dict::DictRef;
        if let Some(dict) = DictRef::from_value(self.fields.to_value()) {
            dict.get_str(attribute).is_some()
        } else if let Ok(iter) = self.fields.to_value().iterate(heap) {
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
        if let Some(dict) = DictRef::from_value(self.fields.to_value()) {
            dict.get_str(attribute)
        } else {
            let key = heap.alloc_str(attribute);
            self.fields.to_value().at(key.to_value(), heap).ok()
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        use starlark::values::dict::DictRef;
        DictRef::from_value(self.fields.to_value())
            .map(|dict| {
                dict.iter()
                    .filter_map(|(k, _)| k.unpack_str().map(|s| s.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// ExecutionInfo - Provider for specifying test execution requirements
// ============================================================================

/// ExecutionInfo provider callable.
///
/// `testing.ExecutionInfo` is a provider callable that specifies execution
/// requirements for tests. Rules return instances of this provider to declare
/// that their tests need specific execution environment settings.
///
/// Reference: https://bazel.build/rules/lib/providers/ExecutionInfo
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ExecutionInfoProvider;

impl ExecutionInfoProvider {
    /// Get the static provider ID for ExecutionInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "ExecutionInfo".to_owned(),
            })
        })
    }
}

impl Display for ExecutionInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider ExecutionInfo>")
    }
}

starlark_simple_value!(ExecutionInfoProvider);

impl ProviderCallableLike for ExecutionInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "ExecutionInfo")]
impl<'v> StarlarkValue<'v> for ExecutionInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        use starlark::values::dict::AllocDict;
        // Extract requirements kwarg, default to empty dict
        let kwargs = args.names_map()?;
        let requirements = kwargs
            .iter()
            .find_map(|(k, v)| {
                if k.as_str() == "requirements" {
                    Some(*v)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                eval.heap()
                    .alloc(AllocDict(std::iter::empty::<(&str, Value)>()))
            });
        Ok(eval.heap().alloc(ExecutionInfoInstance { requirements }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }

    // Any two ExecutionInfoProvider values represent the same provider
    // callable. Without this, rules_python's
    // `IS_BAZEL_6_OR_HIGHER = testing.ExecutionInfo == testing.ExecutionInfo`
    // returns False (each access allocates a fresh unit struct) and the code
    // falls through to `native.py_runtime` which we don't provide.
    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        Ok(other.downcast_ref::<ExecutionInfoProvider>().is_some())
    }
}

/// An instance of ExecutionInfo created by `testing.ExecutionInfo(requirements = {...})`.
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
pub struct ExecutionInfoInstanceGen<V: ValueLifetimeless> {
    /// Requirements dict mapping string keys to string values.
    requirements: V,
}

starlark_complex_value!(pub ExecutionInfoInstance);

impl<V: ValueLifetimeless> Display for ExecutionInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExecutionInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for ExecutionInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        ExecutionInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![("requirements", self.requirements.to_value())]
    }
}

#[starlark_value(type = "ExecutionInfo")]
impl<'v, V: ValueLike<'v> + 'v> StarlarkValue<'v> for ExecutionInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = ExecutionInfoInstance<'v>;

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "requirements" => Some(self.requirements.to_value()),
            _ => None,
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "requirements")
    }
}

// ============================================================================
// RunEnvironmentInfo - Provider for specifying run/test environment variables
// ============================================================================

/// RunEnvironmentInfo provider callable.
///
/// `RunEnvironmentInfo` is a provider for specifying environment variables
/// to be set when running binaries (`kuro run`) or tests (`kuro test`).
///
/// Reference: https://bazel.build/rules/lib/providers/RunEnvironmentInfo
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RunEnvironmentInfoProvider;

impl RunEnvironmentInfoProvider {
    /// Get the static provider ID for RunEnvironmentInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "RunEnvironmentInfo".to_owned(),
            })
        })
    }
}

impl Display for RunEnvironmentInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RunEnvironmentInfo")
    }
}

starlark_simple_value!(RunEnvironmentInfoProvider);

#[starlark_value(type = "RunEnvironmentInfo")]
impl<'v> StarlarkValue<'v> for RunEnvironmentInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        use starlark::values::dict::AllocDict;
        let kwargs = args.names_map()?;
        let environment = kwargs
            .iter()
            .find_map(|(k, v)| {
                if k.as_str() == "environment" {
                    Some(*v)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                eval.heap()
                    .alloc(AllocDict(std::iter::empty::<(&str, Value)>()))
            });
        let inherited_environment = kwargs
            .iter()
            .find_map(|(k, v)| {
                if k.as_str() == "inherited_environment" {
                    Some(*v)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| eval.heap().alloc(AllocList::EMPTY));
        Ok(eval.heap().alloc(RunEnvironmentInfoInstance {
            environment,
            inherited_environment,
        }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

impl ProviderCallableLike for RunEnvironmentInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(RunEnvironmentInfoProvider::provider_id())
    }
}

/// An instance of RunEnvironmentInfo.
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
pub struct RunEnvironmentInfoInstanceGen<V: ValueLifetimeless> {
    /// Environment variable dict mapping string keys to string values.
    environment: V,
    /// List of environment variable names to inherit from the host.
    inherited_environment: V,
}

starlark_complex_value!(pub RunEnvironmentInfoInstance);

impl<V: ValueLifetimeless> Display for RunEnvironmentInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RunEnvironmentInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for RunEnvironmentInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        RunEnvironmentInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![
            ("environment", self.environment.to_value()),
            (
                "inherited_environment",
                self.inherited_environment.to_value(),
            ),
        ]
    }
}

#[starlark_value(type = "RunEnvironmentInfo")]
impl<'v, V: ValueLike<'v> + 'v> StarlarkValue<'v> for RunEnvironmentInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = RunEnvironmentInfoInstance<'v>;

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "environment" => Some(self.environment.to_value()),
            "inherited_environment" => Some(self.inherited_environment.to_value()),
            _ => None,
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "environment" | "inherited_environment")
    }
}

// ============================================================================
// PackageSpecificationInfo - Bazel provider for package visibility
// ============================================================================

/// PackageSpecificationInfo provider callable.
///
/// In Bazel, PackageSpecificationInfo is used for package visibility allowlisting,
/// primarily by cc_toolchain.bzl. It stores package specifications that determine
/// which packages can access a target.
///
/// Reference: https://bazel.build/rules/lib/providers/PackageSpecificationInfo
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PackageSpecificationInfoProvider;

impl PackageSpecificationInfoProvider {
    /// Get the static provider ID for PackageSpecificationInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "PackageSpecificationInfo".to_owned(),
            })
        })
    }
}

impl Display for PackageSpecificationInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PackageSpecificationInfo")
    }
}

starlark_simple_value!(PackageSpecificationInfoProvider);

#[starlark_value(type = "PackageSpecificationInfo")]
impl<'v> StarlarkValue<'v> for PackageSpecificationInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let kwargs = args.names_map()?;
        let packages = kwargs
            .iter()
            .find_map(|(k, v)| {
                if k.as_str() == "packages" {
                    Some(*v)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| eval.heap().alloc(AllocList::EMPTY));
        Ok(eval
            .heap()
            .alloc(PackageSpecificationInfoInstanceGen { packages }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

impl ProviderCallableLike for PackageSpecificationInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(PackageSpecificationInfoProvider::provider_id())
    }
}

/// An instance of PackageSpecificationInfo.
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
pub struct PackageSpecificationInfoInstanceGen<V: ValueLifetimeless> {
    /// List of package specifications (strings like "//pkg", "//pkg/...").
    pub packages: V,
}

starlark_complex_value!(pub PackageSpecificationInfoInstance);

impl<V: ValueLifetimeless> Display for PackageSpecificationInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PackageSpecificationInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for PackageSpecificationInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        PackageSpecificationInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![("packages", self.packages.to_value())]
    }
}

#[starlark_value(type = "PackageSpecificationInfo")]
impl<'v, V: ValueLike<'v> + 'v> StarlarkValue<'v> for PackageSpecificationInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = PackageSpecificationInfoInstance<'v>;

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "packages" => Some(self.packages.to_value()),
            _ => None,
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "packages")
    }
}

// ============================================================================
// LinkerInputStub - Stub for linker input
// ============================================================================

/// A stub for LinkerInput used by cc_common.create_linker_input.
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
pub struct LinkerInputStubGen<V: ValueLifetimeless> {
    pub(crate) owner: V,
    pub(crate) libraries: V,
    pub(crate) user_link_flags: V,
    pub(crate) additional_inputs: V,
}

starlark_complex_value!(pub LinkerInputStub);

impl<V: ValueLifetimeless> Display for LinkerInputStubGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<LinkerInput>")
    }
}

#[starlark::values::starlark_value(type = "LinkerInput")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LinkerInputStubGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "owner" | "libraries" | "user_link_flags" | "additional_inputs"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "owner" => Some(self.owner.to_value()),
            "libraries" => {
                // Return the libraries value (could be a depset or list)
                if self.libraries.to_value().is_none() {
                    Some(heap.alloc(starlark::values::list::AllocList::EMPTY))
                } else {
                    Some(self.libraries.to_value())
                }
            }
            "user_link_flags" => {
                if self.user_link_flags.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.user_link_flags.to_value())
                }
            }
            "additional_inputs" => {
                if self.additional_inputs.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.additional_inputs.to_value())
                }
            }
            _ => None,
        }
    }
}

// ============================================================================
// LinkingContextWithInputs - Linking context with actual linker inputs
// ============================================================================

/// A linking context that stores actual linker inputs.
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
pub struct LinkingContextWithInputsGen<V: ValueLifetimeless> {
    pub(crate) linker_inputs: V,
}

starlark_complex_value!(pub LinkingContextWithInputs);

impl<V: ValueLifetimeless> Display for LinkingContextWithInputsGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<LinkingContext>")
    }
}

#[starlark::values::starlark_value(type = "LinkingContext")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LinkingContextWithInputsGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "linker_inputs")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "linker_inputs" => {
                if self.linker_inputs.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.linker_inputs.to_value())
                }
            }
            _ => None,
        }
    }
}
