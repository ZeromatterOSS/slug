/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible cc_common module and CcInfo provider.
//!
//! This provides an implementation of Bazel's cc_common built-in module
//! that rules_cc (0.2.16+) requires for C/C++ compilation support.
//!
//! For Bazel 9.0+, rules_cc is almost entirely pure Starlark. The key native
//! requirement is this cc_common module which provides:
//! - `internal_DO_NOT_USE()` - Returns internal API struct
//! - Public API functions for toolchain/action configuration
//!
//! Reference: thoughts/shared/research/2026-01-26-rules-cc-native-requirements.md

use std::fmt;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use kuro_core::provider::id::ProviderId;
use kuro_interpreter::types::provider::callable::ProviderCallableLike;
use starlark::coerce::Coerce;
use starlark::collections::SmallMap;
use starlark::collections::StarlarkHasher;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
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
use starlark::values::dict::Dict;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocList;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::context::AnalysisActions;
use crate::interpreter::rule_defs::fragments::ConfigurationFragments;
use crate::interpreter::rule_defs::provider::ProviderLike;

// ============================================================================
// FeatureConfiguration - C++ feature configuration
// ============================================================================

/// FeatureConfiguration holds the enabled features for C++ compilation.
///
/// This is created by cc_common.configure_features() and used to control
/// which compiler flags and behaviors are enabled.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct FeatureConfiguration {
    /// Whether PIC is enabled
    pub pic_enabled: bool,
    /// Whether to use preprocessor defines
    pub defines_enabled: bool,
}

impl Default for FeatureConfiguration {
    fn default() -> Self {
        Self {
            pic_enabled: true,
            defines_enabled: true,
        }
    }
}

impl Display for FeatureConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FeatureConfiguration(pic={})", self.pic_enabled)
    }
}

starlark_simple_value!(FeatureConfiguration);

#[starlark_value(type = "FeatureConfiguration")]
impl<'v> StarlarkValue<'v> for FeatureConfiguration {}

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
    headers: V,
    /// Include directories
    includes: V,
    /// Defines
    defines: V,
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
            "includes" | "quote_includes" | "system_includes" | "framework_includes"
            | "external_includes" => {
                if self.includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.includes.to_value())
                }
            }
            "defines" | "local_defines" => {
                if self.defines.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.defines.to_value())
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
    vars: V,
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
// CtxCheatStub - Stub for actions2ctx_cheat return value
// ============================================================================

/// A stub context returned by actions2ctx_cheat (used when no real actions available).
///
/// This provides the minimum attributes needed by rules_cc's compile function.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatStub;

impl Display for CtxCheatStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx_cheat_stub>")
    }
}

starlark_simple_value!(CtxCheatStub);

#[starlark_value(type = "ctx_cheat_stub")]
impl<'v> StarlarkValue<'v> for CtxCheatStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "label"
                | "bin_dir"
                | "genfiles_dir"
                | "configuration"
                | "actions"
                | "fragments"
                | "workspace_name"
                | "exec_groups"
                | "toolchains"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "label" => Some(heap.alloc(CtxCheatLabelStub)),
            "bin_dir" => Some(heap.alloc(CtxCheatDirStub {
                path: "bazel-out/k8-fastbuild/bin".to_owned(),
            })),
            "genfiles_dir" => Some(heap.alloc(CtxCheatDirStub {
                path: "bazel-out/k8-fastbuild/genfiles".to_owned(),
            })),
            "configuration" => Some(heap.alloc(CtxCheatConfigStub)),
            "actions" => Some(heap.alloc(CtxCheatActionsStub)),
            "fragments" => Some(heap.alloc(ConfigurationFragments::default())),
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            "exec_groups" => {
                Some(heap.alloc(crate::interpreter::rule_defs::context::ExecGroupsDict))
            }
            "toolchains" => {
                Some(heap.alloc(crate::interpreter::rule_defs::context::ToolchainsStub))
            }
            _ => None,
        }
    }
}

/// A context wrapper returned by actions2ctx_cheat that preserves the real actions.
///
/// This wraps the real AnalysisActions so that create_cc_compile_action can
/// use them to register actual compile actions.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxCheatWithActions<'v> {
    /// The real actions object (AnalysisActions)
    actions: Value<'v>,
}

impl<'v> Display for CtxCheatWithActions<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx_cheat_with_actions>")
    }
}

#[starlark_value(type = "ctx_cheat_stub")]
impl<'v> StarlarkValue<'v> for CtxCheatWithActions<'v> {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "label"
                | "bin_dir"
                | "genfiles_dir"
                | "configuration"
                | "actions"
                | "fragments"
                | "workspace_name"
                | "exec_groups"
                | "toolchains"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "label" => Some(heap.alloc(CtxCheatLabelStub)),
            "bin_dir" => Some(heap.alloc(CtxCheatDirStub {
                path: "bazel-out/k8-fastbuild/bin".to_owned(),
            })),
            "genfiles_dir" => Some(heap.alloc(CtxCheatDirStub {
                path: "bazel-out/k8-fastbuild/genfiles".to_owned(),
            })),
            "configuration" => Some(heap.alloc(CtxCheatConfigStub)),
            // Return the REAL actions object here
            "actions" => Some(self.actions),
            "fragments" => Some(heap.alloc(ConfigurationFragments::default())),
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            "exec_groups" => {
                Some(heap.alloc(crate::interpreter::rule_defs::context::ExecGroupsDict))
            }
            "toolchains" => {
                Some(heap.alloc(crate::interpreter::rule_defs::context::ToolchainsStub))
            }
            _ => None,
        }
    }
}

impl<'v> starlark::values::AllocValue<'v> for CtxCheatWithActions<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// A stub for ctx.actions.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatActionsStub;

impl Display for CtxCheatActionsStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<actions>")
    }
}

starlark_simple_value!(CtxCheatActionsStub);

#[starlark_value(type = "actions")]
impl<'v> StarlarkValue<'v> for CtxCheatActionsStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_actions_stub_methods)
    }
}

#[starlark_module]
fn ctx_cheat_actions_stub_methods(builder: &mut MethodsBuilder) {
    /// Declares a file in the output tree.
    #[allow(unused_variables)]
    fn declare_file<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = NoneType)] sibling: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return a stub artifact
        Ok(heap.alloc(CtxCheatArtifactStub {
            path: filename.to_owned(),
        }))
    }

    /// Declares a directory in the output tree.
    #[allow(unused_variables)]
    fn declare_directory<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = NoneType)] sibling: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CtxCheatArtifactStub {
            path: filename.to_owned(),
        }))
    }

    /// Runs an action (stub implementation).
    #[allow(unused_variables)]
    fn run<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = named, default = NoneType)] mnemonic: Value<'v>,
        #[starlark(require = named, default = NoneType)] executable: Value<'v>,
        #[starlark(require = named, default = NoneType)] arguments: Value<'v>,
        #[starlark(require = named, default = NoneType)] inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] progress_message: Value<'v>,
        #[starlark(require = named, default = NoneType)] resource_set: Value<'v>,
        #[starlark(require = named, default = NoneType)] env: Value<'v>,
        #[starlark(require = named, default = false)] use_default_shell_env: bool,
        #[starlark(require = named, default = NoneType)] execution_requirements: Value<'v>,
        #[starlark(require = named, default = NoneType)] toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] exec_group: Value<'v>,
        #[starlark(require = named, default = NoneType)] tools: Value<'v>,
        #[starlark(require = named, default = NoneType)] input_manifests: Value<'v>,
        #[starlark(require = named, default = NoneType)] unused_inputs_list: Value<'v>,
        #[starlark(require = named, default = NoneType)] shadowed_action: Value<'v>,
    ) -> starlark::Result<NoneType> {
        // Stub: do nothing - just accept the parameters
        Ok(NoneType)
    }
}

/// A stub for artifact root (Bazel compatibility).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatArtifactRootStub {
    path: String,
}

impl Display for CtxCheatArtifactRootStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<root {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatArtifactRootStub);

#[starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for CtxCheatArtifactRootStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        attribute == "path"
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

/// A stub for artifact from ctx.actions.declare_file.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatArtifactStub {
    path: String,
}

impl Display for CtxCheatArtifactStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<artifact {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatArtifactStub);

#[starlark_value(type = "File")]
impl<'v> StarlarkValue<'v> for CtxCheatArtifactStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "path"
                | "short_path"
                | "basename"
                | "extension"
                | "is_source"
                | "root"
                | "is_directory"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            "short_path" => Some(heap.alloc_str(&self.path).to_value()),
            "basename" => {
                let basename = self.path.rsplit('/').next().unwrap_or(&self.path);
                Some(heap.alloc_str(basename).to_value())
            }
            "extension" => {
                let ext = self.path.rsplit('.').next().unwrap_or("");
                Some(heap.alloc_str(ext).to_value())
            }
            "is_source" => Some(Value::new_bool(false)),
            "is_directory" => Some(Value::new_bool(false)),
            "root" => Some(heap.alloc(CtxCheatArtifactRootStub {
                path: "bazel-out/k8-fastbuild/bin".to_owned(),
            })),
            _ => None,
        }
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        match CtxCheatArtifactStub::from_value(other) {
            Some(other) => Ok(self.path == other.path),
            None => Ok(false),
        }
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.path.hash(hasher);
        Ok(())
    }
}

/// A stub for ctx.configuration.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatConfigStub;

impl Display for CtxCheatConfigStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<configuration>")
    }
}

starlark_simple_value!(CtxCheatConfigStub);

#[starlark_value(type = "configuration")]
impl<'v> StarlarkValue<'v> for CtxCheatConfigStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_config_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "coverage_enabled")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "coverage_enabled" => Some(Value::new_bool(false)),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_cheat_config_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns whether sibling repository layout is used.
    fn is_sibling_repository_layout(this: &CtxCheatConfigStub) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }
}

/// A stub for directory paths (bin_dir, genfiles_dir).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatDirStub {
    path: String,
}

impl Display for CtxCheatDirStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<dir {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatDirStub);

#[starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for CtxCheatDirStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "path")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

/// A stub label for the ctx_cheat_stub.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatLabelStub;

impl Display for CtxCheatLabelStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "//stub:stub")
    }
}

starlark_simple_value!(CtxCheatLabelStub);

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for CtxCheatLabelStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_label_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "name" | "package" | "workspace_name")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc_str("stub").to_value()),
            "package" => Some(heap.alloc_str("stub").to_value()),
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_cheat_label_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns a label with the same package but a different name.
    fn same_package_label<'v>(
        this: &CtxCheatLabelStub,
        #[starlark(require = pos)] name: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return a new label stub with the given name
        Ok(heap.alloc(CtxCheatLabelStub))
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
// CcCommonInternal - Internal API returned by internal_DO_NOT_USE()
// ============================================================================

/// Helper: push a path string or its corresponding artifact to the args list.
fn push_path_or_artifact<'v>(
    path_str: &str,
    artifact_map: &std::collections::HashMap<String, Value<'v>>,
    args: &mut Vec<Value<'v>>,
    heap: Heap<'v>,
) {
    if let Some(&artifact) = artifact_map.get(path_str) {
        args.push(artifact);
    } else {
        args.push(heap.alloc_str(path_str).to_value());
    }
}

/// Internal cc_common API struct.
///
/// Returned by `cc_common.internal_DO_NOT_USE()`. Contains internal functions
/// that rules_cc uses for low-level C++ compilation actions.
///
/// Reference: cc/private/cc_internal.bzl in rules_cc
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcCommonInternal;

impl Display for CcCommonInternal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cc_common.internal")
    }
}

starlark_simple_value!(CcCommonInternal);

#[starlark_value(type = "cc_common_internal")]
impl<'v> StarlarkValue<'v> for CcCommonInternal {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_common_internal_methods)
    }
}

/// Internal methods for cc_common.internal_DO_NOT_USE() return value.
///
/// These are used by rules_cc's internal Starlark code.
#[starlark_module]
fn cc_common_internal_methods(builder: &mut MethodsBuilder) {
    /// Creates a C++ compile action.
    ///
    /// This is a native function that registers a compile action with Kuro's
    /// action execution system. It bridges rules_cc's Starlark code to the
    /// native action registration infrastructure.
    #[allow(unused_variables)]
    fn create_cc_compile_action<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] action_construction_context: Value<'v>,
        #[starlark(require = named, default = NoneType)] cc_compilation_context: Value<'v>,
        #[starlark(require = named, default = NoneType)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] copts_filter: Value<'v>,
        #[starlark(require = named, default = NoneType)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_compilation_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_include_scanning_roots: Value<
            'v,
        >,
        #[starlark(require = named, default = NoneType)] source: Value<'v>,
        #[starlark(require = named, default = NoneType)] output_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] diagnostics_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] dotd_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] gcno_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] dwo_file: Value<'v>,
        #[starlark(require = named, default = false)] use_pic: bool,
        #[starlark(require = named, default = NoneType)] lto_indexing_file: Value<'v>,
        #[starlark(require = named)] action_name: NoneOr<&str>,
        #[starlark(require = named, default = NoneType)] compile_build_variables: Value<'v>,
        #[starlark(require = named, default = false)] needs_include_validation: bool,
        #[starlark(require = named, default = NoneType)] toolchain_type: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let heap = eval.heap();

        // Log call for debugging
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[create_cc_compile_action] source={}, output_file={}, action_name={:?}",
                source, output_file, action_name
            );
            let _ = writeln!(
                f,
                "  action_construction_context type: {}",
                action_construction_context.get_type()
            );
        }

        // Validate required parameters
        if source.is_none() || output_file.is_none() {
            // Cannot create compile action without source and output
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cc_common_compile.log")
            {
                let _ = writeln!(f, "  EARLY RETURN: source or output_file is None");
            }
            return Ok(NoneType);
        }

        // Get the actions from action_construction_context
        // The context is a CtxCheatWithActions that has the real actions
        let actions_attr_result = action_construction_context.get_attr("actions", heap);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "  actions attr result: {:?}",
                actions_attr_result
                    .as_ref()
                    .map(|o| o.map(|v| v.to_string()))
            );
        }
        let actions_value = if let Ok(Some(actions)) = actions_attr_result {
            actions
        } else {
            // Fallback: action_construction_context might itself be actions
            action_construction_context
        };
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(f, "  actions_value type: {}", actions_value.get_type());
        }

        // Try to get the run method from actions
        let run_attr_result = actions_value.get_attr("run", heap);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "  run attr result: {:?}",
                run_attr_result.as_ref().map(|o| o.map(|v| v.to_string()))
            );
        }
        let run_method = match run_attr_result {
            Ok(Some(method)) => method,
            _ => {
                // No run method available - this is a stub context
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(f, "  EARLY RETURN: no run method available");
                }
                return Ok(NoneType);
            }
        };

        // Get source path for progress message
        let source_path = source
            .get_attr("path", heap)
            .ok()
            .flatten()
            .and_then(|v| v.unpack_str())
            .unwrap_or("unknown")
            .to_owned();

        // Get the action name for mnemonic/category
        // Convert Bazel action names (with hyphens) to Kuro categories (snake_case)
        let action_name_raw = action_name.into_option().unwrap_or("c-compile");
        let action_name_str = action_name_raw.replace("-", "_");

        // Get compiler path from toolchain if available, otherwise use default
        let compiler_path = if !cc_toolchain.is_none() {
            // Try to get compiler path from toolchain
            cc_toolchain
                .get_attr("compiler_executable", heap)
                .ok()
                .flatten()
                .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
                .unwrap_or_else(|| "/usr/bin/gcc".to_owned())
        } else {
            "/usr/bin/gcc".to_owned()
        };

        // Need to call .as_output() on the output artifact to mark it as an output
        // This is required by Kuro's run() to bind the artifact to an action
        let output_artifact = match output_file.get_attr("as_output", heap) {
            Ok(Some(as_output_method)) => eval
                .eval_function(as_output_method, &[], &[])
                .unwrap_or(output_file),
            _ => output_file,
        };

        // Build the command line arguments list
        // Format: [compiler, -c, source, -o, output, flags...]
        // Use output_artifact (after .as_output()) in the command line
        let mut args_vec: Vec<Value<'v>> = Vec::new();
        args_vec.push(heap.alloc_str(&compiler_path).to_value());
        args_vec.push(heap.alloc_str("-c").to_value());
        args_vec.push(source);
        args_vec.push(heap.alloc_str("-o").to_value());
        args_vec.push(output_artifact); // Use the output artifact, not original output_file

        // Add PIC flag if needed
        if use_pic {
            args_vec.push(heap.alloc_str("-fPIC").to_value());
        }

        // Add dependency file generation flags if dotd_file is specified
        if !dotd_file.is_none() {
            args_vec.push(heap.alloc_str("-MMD").to_value()); // Generate deps, excluding system headers
            args_vec.push(heap.alloc_str("-MF").to_value()); // Output dependency file to specified path
            // Get the path of the dotd_file artifact
            if let Ok(Some(path_method)) = dotd_file.get_attr("as_output", heap) {
                if let Ok(dotd_output) = eval.eval_function(path_method, &[], &[]) {
                    args_vec.push(dotd_output);
                }
            }
        }

        let arguments = heap.alloc(args_vec);

        // Build the outputs list with all output artifacts
        let mut outputs_vec: Vec<Value<'v>> = vec![output_artifact];

        // Helper to add auxiliary output artifact to the outputs list
        macro_rules! add_output {
            ($artifact:expr) => {
                if !$artifact.is_none() {
                    if let Ok(Some(method)) = $artifact.get_attr("as_output", heap) {
                        if let Ok(out) = eval.eval_function(method, &[], &[]) {
                            outputs_vec.push(out);
                        }
                    }
                }
            };
        }

        // Add auxiliary outputs if provided (dotd, diagnostics, gcno, dwo, lto)
        add_output!(dotd_file);
        add_output!(diagnostics_file);
        add_output!(gcno_file);
        add_output!(dwo_file);
        add_output!(lto_indexing_file);

        let outputs_list = heap.alloc(outputs_vec);

        // Build the progress message
        let progress_msg = heap
            .alloc_str(&format!("Compiling {}", source_path))
            .to_value();

        // Build named arguments for run()
        // run(arguments, outputs=outputs, mnemonic=mnemonic, progress_message=msg)
        let named_args: Vec<(&str, Value<'v>)> = vec![
            ("outputs", outputs_list),
            ("mnemonic", heap.alloc_str(&action_name_str).to_value()),
            ("progress_message", progress_msg),
        ];

        // Invoke actions.run() using Starlark's function evaluation
        // This properly registers the action through Kuro's infrastructure
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(f, "  Calling run_method with:");
            let _ = writeln!(f, "    arguments: {}", arguments);
            let _ = writeln!(f, "    outputs: {}", outputs_list);
            let _ = writeln!(f, "    mnemonic: {}", action_name_str);
        }
        let run_result = eval.eval_function(run_method, &[arguments], &named_args);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "  run result: {:?}",
                run_result
                    .as_ref()
                    .map(|v| v.to_string())
                    .map_err(|e| e.to_string())
            );
        }

        Ok(NoneType)
    }

    /// Gets the artifact name for a given category.
    ///
    /// Categories include: "object_file", "pic_object_file", "executable", etc.
    #[allow(unused_variables)]
    fn get_artifact_name_for_category<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] category: &str,
        #[starlark(require = named, default = "")] output_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[get_artifact_name_for_category] category={:?}, output_name={:?}",
                category, output_name
            );
        }
        // TODO(cc_common): Implement proper artifact naming based on toolchain
        // For now, return basic naming conventions
        let name = if output_name.is_empty() {
            "output"
        } else {
            output_name
        };
        // Category names come in both uppercase (from rules_cc artifact_category_names struct)
        // and lowercase (from direct string usage). Normalize to uppercase for matching.
        // Extensions follow Unix/Linux conventions (not Windows .dll/.lib or macOS .dylib).
        let result = match category.to_uppercase().as_str() {
            // Object files
            "OBJECT_FILE" => format!("{}.o", name),
            "PIC_OBJECT_FILE" => format!("{}.pic.o", name),
            "PIC_FILE" => format!("{}.pic", name),

            // Libraries
            "STATIC_LIBRARY" => format!("lib{}.a", name),
            "ALWAYSLINK_STATIC_LIBRARY" => format!("lib{}.lo", name), // GNU libtool convention
            "DYNAMIC_LIBRARY" => format!("lib{}.so", name),
            "INTERFACE_LIBRARY" => format!("lib{}.so", name), // Same as dynamic per Bazel

            // Executables
            "EXECUTABLE" => name.to_owned(),

            // Dependency tracking
            "INCLUDED_FILE_LIST" => format!("{}.d", name),

            // Diagnostics
            "SERIALIZED_DIAGNOSTICS_FILE" => format!("{}.dia", name), // Clang diagnostics

            // Headers
            "GENERATED_HEADER" => format!("{}.h", name),
            "PROCESSED_HEADER" => format!("{}.h", name),

            // C++20 modules
            "CPP_MODULE" => format!("{}.pcm", name),
            "CPP_MODULES_DDI" => format!("{}.ddi", name),
            "CPP_MODULES_INFO" => format!("{}.modinfo", name),
            "CPP_MODULES_MODMAP" => format!("{}.modmap", name),
            "CPP_MODULES_MODMAP_INPUT" => format!("{}.input_modmap", name),

            // Preprocessing
            "PREPROCESSED_C_SOURCE" => format!("{}.i", name),
            "PREPROCESSED_CPP_SOURCE" => format!("{}.ii", name),

            // Coverage (gcov)
            "COVERAGE_DATA_FILE" => format!("{}.gcno", name),
            "COVERAGE_NOTES_FILE" => format!("{}.gcda", name),

            // Other
            "CLIF_OUTPUT_PROTO" => format!("{}.opb", name),

            // Unknown category - use category as extension
            _ => format!("{}.{}", name, category),
        };
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(f, "  result = {:?}", result);
        }
        Ok(result)
    }

    /// Combines toolchain variables from multiple sources.
    ///
    /// Takes 2 or 3 positional arguments - base variables plus 1-2 override variables.
    /// Variables are merged, with later arguments taking precedence.
    #[allow(unused_variables)]
    fn combine_cc_toolchain_variables<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] base: Value<'v>,
        #[starlark(require = pos)] first_override: Value<'v>,
        #[starlark(default = NoneType)] second_override: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // For now, return the last non-None variables (last one wins)
        // TODO(cc_common): Implement proper merging of variables
        if !second_override.is_none() {
            Ok(second_override)
        } else if !first_override.is_none() {
            Ok(first_override)
        } else {
            Ok(base)
        }
    }

    /// Gets the rule context from an actions object.
    ///
    /// This is a workaround used by rules_cc to access ctx from actions.
    /// We preserve the real actions object so create_cc_compile_action can use it.
    #[allow(unused_variables)]
    fn actions2ctx_cheat<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return a wrapper that preserves the real actions object
        // This allows create_cc_compile_action to register real actions
        Ok(eval.heap().alloc(CtxCheatWithActions { actions }))
    }

    /// Creates CcToolchainVariables from a dictionary.
    #[allow(unused_variables)]
    fn cc_toolchain_variables<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] vars: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Wrap the variables dict in CcToolchainVariables
        Ok(eval.heap().alloc(CcToolchainVariablesGen { vars }))
    }

    /// Freezes a list to an immutable tuple.
    fn freeze<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        value: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Properly convert list to tuple for immutability
        // For now, just return the value as-is since this is a stub
        Ok(value)
    }

    /// Returns the execution requirements for a given action.
    ///
    /// Returns a list of execution requirements (like "requires-worker-protocol:json")
    /// that should be added to actions using the specified tool.
    #[allow(unused_variables)]
    fn get_tool_requirement_for_action<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty list - no special execution requirements
        Ok(eval.heap().alloc(Vec::<String>::new()))
    }

    /// Creates a tree artifact compile action template.
    fn create_cc_compile_action_template<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement tree artifact compile template
        Ok(NoneType)
    }

    /// Wraps link actions for platform compatibility.
    ///
    /// Arguments:
    /// - actions: The ctx.actions object
    /// - build_config: Build configuration (usually ctx.configuration), optional
    /// - use_shareable_artifact_factory: Whether to use shareable artifact factory, optional
    #[allow(unused_variables)]
    fn wrap_link_actions<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(default = NoneType)] build_config: Value<'v>,
        #[starlark(default = false)] use_shareable_artifact_factory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement link action wrapping
        // Return a wrapper that proxies the actions object
        Ok(actions)
    }

    /// Gets the SONAME for a dynamic library.
    #[allow(unused_variables)]
    fn dynamic_library_soname<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] short_path: &str,
        #[starlark(require = pos)] preserve_name: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // Extract library name from the path for SONAME
        let basename = short_path.rsplit('/').next().unwrap_or(short_path);
        Ok(basename.to_owned())
    }

    /// Creates a symlink for a dynamic library.
    #[allow(unused_variables)]
    fn dynamic_library_symlink<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] artifact: Value<'v>,
        #[starlark(require = pos)] solib_dir: Value<'v>,
        #[starlark(require = pos)] preserve_name: bool,
        #[starlark(require = pos)] use_short_path: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return the artifact unchanged - symlink creation is a stub
        Ok(artifact)
    }

    /// Interns a sequence for efficiency (returns it unchanged).
    #[allow(unused_variables)]
    fn intern_seq<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] value: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return the sequence unchanged - interning is just an optimization
        Ok(value)
    }

    /// Gets link arguments for a given feature configuration.
    ///
    /// This function extracts variables from build_variables and constructs
    /// the linker command line arguments. For rules_cc compatibility, this
    /// returns an Args-like list that can be passed to actions.run(arguments=...).
    ///
    /// The build_variables contain `libraries_to_link` which is a list of
    /// provider instances created by rules_cc:
    /// - _NamedLibraryInfo: type in {object_file, static_library, dynamic_library, interface_library}
    /// - _ObjectFileGroupInfo: type = object_file_group, has .object_files list
    /// - _VersionedLibraryInfo: type = versioned_dynamic_library, has .name and .path
    ///
    /// For dynamic_library type, .name is a short library name (e.g., "hello_lib")
    /// that should be emitted as -l<name>. For other types, .name is a full path.
    #[allow(unused_variables)]
    fn get_link_args<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: Value<'v>,
        #[starlark(require = named)] build_variables: Value<'v>,
        #[starlark(require = named, default = NoneType)] parameter_file_type: Value<'v>,
        // Kuro extension: Optional input artifacts for proper path resolution.
        #[starlark(require = named, default = NoneType)] input_artifacts: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Debug logging
        use std::io::Write;
        let mut debug_log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
            .ok();
        if let Some(ref mut f) = debug_log {
            let _ = writeln!(f, "[get_link_args] action_name={}", action_name);
        }

        // Get action name as string
        let action_name_str = action_name.unpack_str().unwrap_or("c++-link-executable");

        let mut args: Vec<Value<'v>> = Vec::new();

        // Helper to get a variable value from either CcToolchainVariables or a raw dict
        let get_var = |key: &str| -> Option<Value<'v>> {
            if let Some(v) = build_variables.get_attr(key, heap).ok().flatten() {
                return Some(v);
            }
            if let Some(dict_ref) = DictRef::from_value(build_variables) {
                if let Some(v) = dict_ref.get_str(key) {
                    return Some(v);
                }
            }
            None
        };

        // Build a map from artifact paths to artifact values (for resolving string paths)
        let mut artifact_map: std::collections::HashMap<String, Value<'v>> =
            std::collections::HashMap::new();
        if !input_artifacts.is_none() {
            let artifacts_iter =
                if let Ok(Some(to_list)) = input_artifacts.get_attr("to_list", heap) {
                    if let Ok(list_val) = eval.eval_function(to_list, &[], &[]) {
                        list_val.iterate(heap).ok()
                    } else {
                        None
                    }
                } else {
                    input_artifacts.iterate(heap).ok()
                };
            if let Some(iter) = artifacts_iter {
                for artifact in iter {
                    if let Ok(Some(short_path)) = artifact.get_attr("short_path", heap) {
                        if let Some(path_str) = short_path.unpack_str() {
                            artifact_map.insert(path_str.to_owned(), artifact);
                        }
                    }
                    if let Ok(Some(path_attr)) = artifact.get_attr("path", heap) {
                        if let Some(path_str) = path_attr.unpack_str() {
                            artifact_map.insert(path_str.to_owned(), artifact);
                        }
                    }
                }
            }
        }

        // --- Output path ---
        if let Some(output) = get_var("output_execpath") {
            if action_name_str.contains("static-library") {
                args.push(heap.alloc_str("rcs").to_value());
            } else if action_name_str.contains("dynamic-library") {
                args.push(heap.alloc_str("-shared").to_value());
                args.push(heap.alloc_str("-o").to_value());
            } else {
                args.push(heap.alloc_str("-o").to_value());
            }

            if output.unpack_str().is_some() {
                args.push(output);
            } else {
                let path_result = output.get_attr("path", heap);
                if let Ok(Some(as_output_method)) = output.get_attr("as_output", heap) {
                    match eval.eval_function(as_output_method, &[], &[]) {
                        Ok(output_artifact) => {
                            args.push(output_artifact);
                        }
                        Err(_) => {
                            if let Ok(Some(path)) = path_result {
                                args.push(path);
                            } else {
                                args.push(heap.alloc_str(&output.to_str()).to_value());
                            }
                        }
                    }
                } else if let Ok(Some(path)) = path_result {
                    args.push(path);
                } else {
                    args.push(heap.alloc_str(&output.to_str()).to_value());
                }
            }
        }

        // Helper: iterate a value that may be a list or depset
        // For depsets, call .to_list() first to get an iterable
        let iterate_value =
            |val: Value<'v>, eval_ref: &mut Evaluator<'v, '_, '_>| -> Vec<Value<'v>> {
                let h = eval_ref.heap();
                // Try to_list() for depsets
                if let Ok(Some(to_list_method)) = val.get_attr("to_list", h) {
                    if let Ok(list_val) = eval_ref.eval_function(to_list_method, &[], &[]) {
                        if let Ok(iter) = list_val.iterate(h) {
                            return iter.collect();
                        }
                    }
                }
                // Fall back to direct iteration (for lists)
                if let Ok(iter) = val.iterate(h) {
                    iter.collect()
                } else {
                    Vec::new()
                }
            };

        // --- Library search directories (-L flags and -rpath) ---
        // Must come before libraries_to_link so the linker can find -l libraries.
        // Also add -rpath for the same directories so the runtime linker can find them.
        let mut lib_search_dirs: Vec<String> = Vec::new();
        if let Some(dirs) = get_var("library_search_directories") {
            for dir in iterate_value(dirs, eval) {
                if let Some(dir_str) = dir.unpack_str() {
                    if !dir_str.is_empty() {
                        args.push(heap.alloc_str(&format!("-L{}", dir_str)).to_value());
                        lib_search_dirs.push(dir_str.to_owned());
                    }
                }
            }
        }

        // --- Libraries to link ---
        // Process based on .type field from rules_cc provider instances
        if let Some(libs) = get_var("libraries_to_link") {
            if let Ok(iter) = libs.iterate(heap) {
                for lib in iter {
                    // Get the library type to determine how to format the argument
                    let lib_type = lib
                        .get_attr("type", heap)
                        .ok()
                        .flatten()
                        .and_then(|v| v.unpack_str().map(|s| s.to_owned()));

                    let is_whole_archive = lib
                        .get_attr("is_whole_archive", heap)
                        .ok()
                        .flatten()
                        .map(|v| v.unpack_bool() == Some(true))
                        .unwrap_or(false);

                    if is_whole_archive {
                        args.push(heap.alloc_str("-Wl,--whole-archive").to_value());
                    }

                    match lib_type.as_deref() {
                        Some("dynamic_library") => {
                            // Dynamic library: emit -l<name> flag
                            // .name is a short name like "hello_lib" (from "libhello_lib.so")
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    args.push(
                                        heap.alloc_str(&format!("-l{}", name_str)).to_value(),
                                    );
                                }
                            }
                        }
                        Some("versioned_dynamic_library") => {
                            // Versioned dynamic library: use -l:<name> for exact match
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    args.push(
                                        heap.alloc_str(&format!("-l:{}", name_str)).to_value(),
                                    );
                                }
                            }
                        }
                        Some("object_file_group") => {
                            // Object file group: iterate .object_files and add each
                            if let Some(object_files) =
                                lib.get_attr("object_files", heap).ok().flatten()
                            {
                                if let Ok(obj_iter) = object_files.iterate(heap) {
                                    for obj in obj_iter {
                                        if obj.get_type() == "File" {
                                            args.push(obj);
                                        } else if let Some(path_str) = obj.unpack_str() {
                                            push_path_or_artifact(
                                                path_str,
                                                &artifact_map,
                                                &mut args,
                                                heap,
                                            );
                                        } else {
                                            args.push(obj);
                                        }
                                    }
                                }
                            }
                        }
                        Some("object_file")
                        | Some("static_library")
                        | Some("interface_library") => {
                            // These types use .name as a full path
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    push_path_or_artifact(name_str, &artifact_map, &mut args, heap);
                                } else {
                                    args.push(name);
                                }
                            }
                        }
                        _ => {
                            // Unknown type or no type field - use legacy fallback
                            if let Some(path_str) = lib.unpack_str() {
                                push_path_or_artifact(path_str, &artifact_map, &mut args, heap);
                            } else if let Some(artifact) =
                                lib.get_attr("artifact", heap).ok().flatten()
                            {
                                if artifact.is_none() {
                                    if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                        if let Some(name_str) = name.unpack_str() {
                                            push_path_or_artifact(
                                                name_str,
                                                &artifact_map,
                                                &mut args,
                                                heap,
                                            );
                                        } else {
                                            args.push(name);
                                        }
                                    }
                                } else {
                                    args.push(artifact);
                                }
                            } else if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    push_path_or_artifact(name_str, &artifact_map, &mut args, heap);
                                } else {
                                    args.push(name);
                                }
                            } else if lib.get_type() == "File" {
                                args.push(lib);
                            } else {
                                let path_str = lib.to_str();
                                push_path_or_artifact(&path_str, &artifact_map, &mut args, heap);
                            }
                        }
                    }

                    if is_whole_archive {
                        args.push(heap.alloc_str("-Wl,--no-whole-archive").to_value());
                    }
                }
            }
        }

        // --- User link flags ---
        if let Some(flags) = get_var("user_link_flags") {
            if let Ok(iter) = flags.iterate(heap) {
                for flag in iter {
                    if flag.unpack_str().is_some() {
                        args.push(flag);
                    }
                }
            }
        }

        // --- Runtime library search directories (-rpath flags) ---
        // Use $ORIGIN-relative paths so the runtime linker can find shared libraries
        // regardless of the working directory when the binary is executed.
        let output_dir: Option<String> = get_var("output_execpath").and_then(|v| {
            let path_str = if let Some(s) = v.unpack_str() {
                s.to_owned()
            } else if let Ok(Some(path_attr)) = v.get_attr("path", heap) {
                path_attr
                    .unpack_str()
                    .map(|s| s.to_owned())
                    .unwrap_or_else(|| v.to_str())
            } else {
                v.to_str()
            };
            std::path::Path::new(&path_str)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
        });

        let make_origin_rpath = |dir_str: &str| -> String {
            if let Some(ref out_dir) = output_dir {
                // Normalize: strip leading "../" components from dir_str.
                // rules_cc may produce relative paths like "../../../buck-out/v2/gen/..."
                // but we need the project-root-relative path for $ORIGIN computation.
                let mut normalized = dir_str;
                while normalized.starts_with("../") {
                    normalized = &normalized[3..];
                }
                if normalized == ".." {
                    normalized = "";
                }

                // Compute relative path from binary's directory to the library directory
                let from_components: Vec<&str> =
                    out_dir.split('/').filter(|s| !s.is_empty()).collect();
                let to_components: Vec<&str> =
                    normalized.split('/').filter(|s| !s.is_empty()).collect();
                let common = from_components
                    .iter()
                    .zip(to_components.iter())
                    .take_while(|(a, b)| a == b)
                    .count();
                let up_count = from_components.len() - common;
                let mut rel = String::from("$ORIGIN");
                for _ in 0..up_count {
                    rel.push_str("/..");
                }
                for &component in &to_components[common..] {
                    rel.push('/');
                    rel.push_str(component);
                }
                format!("-Wl,-rpath,{}", rel)
            } else {
                // No output path available, use as-is (fallback)
                format!("-Wl,-rpath,{}", dir_str)
            }
        };

        let mut has_rpath = false;
        let mut seen_rpaths: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Some(dirs) = get_var("runtime_library_search_directories") {
            for dir in iterate_value(dirs, eval) {
                if let Some(dir_str) = dir.unpack_str() {
                    if !dir_str.is_empty() {
                        let rpath = make_origin_rpath(dir_str);
                        if seen_rpaths.insert(rpath.clone()) {
                            args.push(heap.alloc_str(&rpath).to_value());
                        }
                        has_rpath = true;
                    }
                }
            }
        }
        // Fallback: use library_search_directories for rpath if no explicit rpath dirs
        if !has_rpath && !lib_search_dirs.is_empty() {
            for dir_str in &lib_search_dirs {
                let rpath = make_origin_rpath(dir_str);
                if seen_rpaths.insert(rpath.clone()) {
                    args.push(heap.alloc_str(&rpath).to_value());
                }
            }
        }

        Ok(heap.alloc(args))
    }

    /// Declares a compile output file.
    ///
    /// This function uses the real AnalysisActions from the ctx parameter
    /// to create a properly registered output artifact.
    fn declare_compile_output_file<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] label: Value<'v>,
        #[starlark(require = named, default = "")] output_name: &str,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[declare_compile_output_file] output_name={:?}",
                output_name
            );
        }

        let heap = eval.heap();
        let _ = (label, configuration); // Unused for now

        // Get the real actions from ctx.actions
        let actions_value = match ctx.get_attr("actions", heap) {
            Ok(Some(actions)) => actions,
            _ => {
                // Fallback to stub if no real actions available
                return Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.to_owned(),
                }));
            }
        };

        // Try to get the declare_file method
        let declare_file_method = match actions_value.get_attr("declare_file", heap) {
            Ok(Some(method)) => method,
            _ => {
                // Fallback to stub if declare_file not available
                return Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.to_owned(),
                }));
            }
        };

        // Call declare_file(output_name) using Starlark's function evaluation
        let filename = heap.alloc_str(output_name).to_value();
        match eval.eval_function(declare_file_method, &[filename], &[]) {
            Ok(artifact) => {
                // Log the artifact's path attribute
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(f, "  declared artifact: {}", artifact);
                    if let Ok(Some(path)) = artifact.get_attr("path", heap) {
                        let _ = writeln!(f, "  artifact.path = {}", path);
                    }
                }
                Ok(artifact)
            }
            Err(e) => {
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(f, "  declare_file error: {}", e);
                }
                // Fallback to stub on error
                Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.to_owned(),
                }))
            }
        }
    }

    /// Declares an auxiliary output file (dwo, gcno, etc.).
    #[allow(unused_variables)]
    fn declare_other_output_file<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] source_file: Value<'v>,
        #[starlark(require = named, default = "")] extension: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement other output declaration
        Ok(NoneType)
    }

    /// Checks if an artifact is a tree artifact.
    fn is_tree_artifact<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        _artifact: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // TODO(cc_common): Check actual artifact type
        Ok(false)
    }

    /// Computes the output name prefix directory.
    ///
    /// This returns the directory prefix for object files, typically `_objs/{purpose}`.
    /// In Bazel, this creates object files in a target-specific subdirectory.
    #[allow(unused_variables)]
    fn compute_output_name_prefix_dir<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] purpose: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(f, "[compute_output_name_prefix_dir] purpose={}", purpose);
        }

        // The purpose is typically the target name or a unique identifier.
        // Object files should go in `_objs/{purpose}/` directory.
        if purpose.is_none() {
            // No purpose specified, use a default
            return Ok("_objs".to_owned());
        }

        // Try to get a string value from purpose
        if let Some(purpose_str) = purpose.unpack_str() {
            // If purpose is empty string, return just "_objs" without trailing slash
            // to avoid double slashes like "_objs//main.o"
            if purpose_str.is_empty() {
                return Ok("_objs".to_owned());
            }
            return Ok(format!("_objs/{}", purpose_str));
        }

        // If purpose has a 'name' attribute (like a Label), use that
        if let Ok(Some(name)) = purpose.get_attr("name", eval.heap()) {
            if let Some(name_str) = name.unpack_str() {
                if name_str.is_empty() {
                    return Ok("_objs".to_owned());
                }
                return Ok(format!("_objs/{}", name_str));
            }
        }

        // Fallback: just use _objs
        Ok("_objs".to_owned())
    }

    /// Interns a string sequence variable value for efficiency.
    fn intern_string_sequence_variable_value<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        value: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // For now, just return the value as-is
        // TODO(cc_common): Implement proper interning
        Ok(value)
    }

    /// Gets per-file compile options.
    #[allow(unused_variables)]
    fn per_file_copts<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] cpp_configuration: Value<'v>,
        #[starlark(require = pos)] source_file: Value<'v>,
        #[starlark(require = pos)] label: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement per-file copts
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    /// Checks access to private API (allowlist enforcement).
    #[allow(unused_variables)]
    fn check_private_api<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] allowlist: Value<'v>,
        #[starlark(require = named, default = 1)] depth: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // Always allow for now
        Ok(true)
    }

    /// Creates a HeaderInfo struct.
    #[allow(unused_variables)]
    fn create_header_info<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] header_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_header_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_public_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_private_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_pic_module: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return a HeaderInfo stub with the necessary attributes
        Ok(eval.heap().alloc(HeaderInfoStub))
    }

    /// Creates a HeaderInfo struct with dependency tracking.
    #[allow(unused_variables)]
    fn create_header_info_with_deps<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = NoneType)] header_info: Value<'v>,
        #[starlark(require = named, default = NoneType)] merged_deps: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper HeaderInfo with deps
        Ok(eval.heap().alloc(HeaderInfoStub))
    }
}

// ============================================================================
// CcCommonModule - The main cc_common module
// ============================================================================

/// The cc_common module provides C/C++ compilation support.
///
/// This is Bazel's native module for C++ build configuration. For Bazel 9.0+,
/// most of the actual compilation logic is in pure Starlark (rules_cc), but
/// the native cc_common module provides low-level primitives.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcCommonModule;

impl Display for CcCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cc_common")
    }
}

starlark_simple_value!(CcCommonModule);

#[starlark_value(type = "cc_common")]
impl<'v> StarlarkValue<'v> for CcCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_common_module_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        // Report which attributes exist for hasattr() checks
        matches!(
            attribute,
            "internal_DO_NOT_USE"
                | "get_tool_for_action"
                | "get_execution_requirements"
                | "action_is_enabled"
                | "get_memory_inefficient_command_line"
                | "get_environment_variables"
                | "empty_variables"
                | "do_not_use_tools_cpp_compiler_present"
                | "CcToolchainInfo"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "do_not_use_tools_cpp_compiler_present" => Some(Value::new_bool(true)),
            "CcToolchainInfo" => Some(heap.alloc(CcToolchainInfoProvider)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "internal_DO_NOT_USE".to_owned(),
            "get_tool_for_action".to_owned(),
            "get_execution_requirements".to_owned(),
            "action_is_enabled".to_owned(),
            "get_memory_inefficient_command_line".to_owned(),
            "get_environment_variables".to_owned(),
            "empty_variables".to_owned(),
            "do_not_use_tools_cpp_compiler_present".to_owned(),
            "CcToolchainInfo".to_owned(),
        ]
    }
}

/// Methods on the cc_common module.
#[starlark_module]
fn cc_common_module_methods(builder: &mut MethodsBuilder) {
    /// Returns the internal cc_common API struct.
    ///
    /// Used by rules_cc via: cc_internal = cc_common.internal_DO_NOT_USE()
    #[starlark(attribute)]
    fn internal_DO_NOT_USE(this: &CcCommonModule) -> starlark::Result<CcCommonInternal> {
        let _ = this;
        Ok(CcCommonInternal)
    }

    /// Configures C++ features based on toolchain and requested features.
    ///
    /// Returns a FeatureConfiguration that controls which compiler flags are enabled.
    #[allow(unused_variables)]
    fn configure_features<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] requested_features: Value<'v>,
        #[starlark(require = named, default = NoneType)] unsupported_features: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<FeatureConfiguration> {
        eprintln!(
            "[cc_common.configure_features] called with ctx={}, cc_toolchain={}",
            ctx, cc_toolchain
        );
        // TODO(cc_common): Properly process features from toolchain and config
        // For now, return a default feature configuration
        Ok(FeatureConfiguration::default())
    }

    /// Compiles C/C++ source files.
    ///
    /// This is the main compilation function that creates compile actions for each
    /// source file and returns compilation context and outputs.
    ///
    /// Returns a tuple of (CcCompilationContext, CompilationOutputs).
    #[allow(unused_variables)]
    fn compile<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = NoneType)] public_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] private_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] loose_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] quote_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] system_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] framework_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] local_defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] include_prefix: Value<'v>,
        #[starlark(require = named, default = NoneType)] strip_include_prefix: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_compile_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] conly_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] cxx_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] compilation_contexts: Value<'v>,
        #[starlark(require = named, default = NoneType)] implementation_compilation_contexts: Value<
            'v,
        >,
        #[starlark(require = named, default = false)] disallow_pic_outputs: bool,
        #[starlark(require = named, default = false)] disallow_nopic_outputs: bool,
        #[starlark(require = named, default = NoneType)] additional_include_scanning_roots: Value<
            'v,
        >,
        #[starlark(require = named, default = false)] do_not_generate_module_map: bool,
        #[starlark(require = named, default = false)] code_coverage_enabled: bool,
        #[starlark(require = named, default = NoneType)] hdrs_checking_mode: Value<'v>,
        #[starlark(require = named, default = NoneType)] variables_extension: Value<'v>,
        #[starlark(require = named, default = NoneType)] language: Value<'v>,
        #[starlark(require = named, default = NoneType)] purpose: Value<'v>,
        #[starlark(require = named, default = NoneType)] copts_filter: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] module_interfaces: Value<'v>,
        #[starlark(require = named, default = NoneType)] non_compilation_additional_inputs: Value<
            'v,
        >,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Write debug to file
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[cc_common.compile] name={}, srcs={}, srcs.is_none()={}",
                name,
                srcs,
                srcs.is_none()
            );
        }

        // Debug logging
        eprintln!(
            "[cc_common.compile] name={}, srcs={}, srcs.is_none()={}",
            name,
            srcs,
            srcs.is_none()
        );

        // Collect source files to compile
        let mut object_files: Vec<Value<'v>> = Vec::new();
        let mut pic_object_files: Vec<Value<'v>> = Vec::new();

        // Get the declare_file method from actions
        let declare_file_method = actions.get_attr("declare_file", heap).ok().flatten();
        let run_method = actions.get_attr("run", heap).ok().flatten();

        eprintln!(
            "[cc_common.compile] declare_file_method={:?}, run_method={:?}",
            declare_file_method.map(|v| v.to_string()),
            run_method.map(|v| v.to_string())
        );

        // Process source files if provided
        // srcs is a list of (Artifact, Label) tuples from cc_helper.get_srcs()
        if !srcs.is_none() {
            eprintln!("[cc_common.compile] Processing srcs...");
            // Try to iterate over srcs
            if let Ok(iter) = srcs.iterate(heap) {
                let items: Vec<_> = iter.collect();
                eprintln!("[cc_common.compile] srcs has {} items", items.len());
                for src_tuple in items {
                    eprintln!("[cc_common.compile] Processing src_tuple: {}", src_tuple);
                    // Extract the artifact from the (Artifact, Label) tuple
                    // Try tuple index first, then fall back to treating it as artifact directly
                    let src = src_tuple
                        .at(heap.alloc(0i32).to_value(), heap)
                        .unwrap_or(src_tuple);

                    // Get source file path
                    let src_path = src
                        .get_attr("path", heap)
                        .ok()
                        .flatten()
                        .and_then(|v| v.unpack_str())
                        .unwrap_or("unknown.c");

                    // Determine output filename (replace extension with .o)
                    let basename = src_path.rsplit('/').next().unwrap_or(src_path);
                    let output_name = if let Some(dot_pos) = basename.rfind('.') {
                        format!("_objs/{}/{}.o", name, &basename[..dot_pos])
                    } else {
                        format!("_objs/{}/{}.o", name, basename)
                    };
                    let pic_output_name = if let Some(dot_pos) = basename.rfind('.') {
                        format!("_objs/{}/{}.pic.o", name, &basename[..dot_pos])
                    } else {
                        format!("_objs/{}/{}.pic.o", name, basename)
                    };

                    // Log what we're about to do
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/tmp/cc_common_compile.log")
                    {
                        let _ = writeln!(
                            f,
                            "  Declaring output: {}, pic: {}",
                            output_name, pic_output_name
                        );
                    }

                    // Declare output files
                    if let Some(declare_file) = declare_file_method {
                        // Regular object file
                        let output_file = eval.eval_function(
                            declare_file,
                            &[heap.alloc_str(&output_name).to_value()],
                            &[],
                        );
                        if let Ok(mut f) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open("/tmp/cc_common_compile.log")
                        {
                            let _ = writeln!(
                                f,
                                "  declare_file result: {:?}",
                                output_file.as_ref().map(|v| v.to_string())
                            );
                        }
                        let output_file = output_file.ok();

                        // PIC object file
                        let pic_output_file = eval
                            .eval_function(
                                declare_file,
                                &[heap.alloc_str(&pic_output_name).to_value()],
                                &[],
                            )
                            .ok();

                        // Register compile action if run method available
                        if let (Some(run), Some(out), Some(pic_out)) =
                            (run_method, output_file, pic_output_file)
                        {
                            // Get output as output artifact
                            let output_artifact = out
                                .get_attr("as_output", heap)
                                .ok()
                                .flatten()
                                .and_then(|method| eval.eval_function(method, &[], &[]).ok())
                                .unwrap_or(out);
                            let pic_output_artifact = pic_out
                                .get_attr("as_output", heap)
                                .ok()
                                .flatten()
                                .and_then(|method| eval.eval_function(method, &[], &[]).ok())
                                .unwrap_or(pic_out);

                            // Build compile command: gcc -c src -o output
                            let args = heap.alloc(vec![
                                heap.alloc_str("/usr/bin/gcc").to_value(),
                                heap.alloc_str("-c").to_value(),
                                src,
                                heap.alloc_str("-o").to_value(),
                                output_artifact,
                            ]);
                            let outputs_list = heap.alloc(vec![output_artifact]);
                            let progress = heap
                                .alloc_str(&format!("Compiling {}", basename))
                                .to_value();

                            // Call actions.run() for regular compile
                            // Use unique identifier to avoid "multiple actions with same category" error
                            let identifier = heap.alloc_str(&format!("{}.o", basename)).to_value();
                            let run_result = eval.eval_function(
                                run,
                                &[args],
                                &[
                                    ("outputs", outputs_list),
                                    ("category", heap.alloc_str("cpp_compile").to_value()),
                                    ("identifier", identifier),
                                    ("progress_message", progress),
                                ],
                            );
                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open("/tmp/cc_common_compile.log")
                            {
                                let _ = writeln!(
                                    f,
                                    "  actions.run result (regular): {:?}",
                                    run_result
                                        .as_ref()
                                        .map(|v| v.to_string())
                                        .map_err(|e| e.to_string())
                                );
                            }

                            // Register PIC compile action with unique identifier
                            let pic_args = heap.alloc(vec![
                                heap.alloc_str("/usr/bin/gcc").to_value(),
                                heap.alloc_str("-c").to_value(),
                                heap.alloc_str("-fPIC").to_value(),
                                src,
                                heap.alloc_str("-o").to_value(),
                                pic_output_artifact,
                            ]);
                            let pic_outputs_list = heap.alloc(vec![pic_output_artifact]);
                            let pic_progress = heap
                                .alloc_str(&format!("Compiling {} (PIC)", basename))
                                .to_value();
                            let pic_identifier =
                                heap.alloc_str(&format!("{}.pic.o", basename)).to_value();

                            let _ = eval.eval_function(
                                run,
                                &[pic_args],
                                &[
                                    ("outputs", pic_outputs_list),
                                    ("category", heap.alloc_str("cpp_compile").to_value()),
                                    ("identifier", pic_identifier),
                                    ("progress_message", pic_progress),
                                ],
                            );

                            object_files.push(out);
                            pic_object_files.push(pic_out);
                        }
                    }
                }
            }
        }

        // Create compilation context
        let none_val = Value::new_none();
        let compilation_context = heap.alloc(CcCompilationContextGen {
            headers: none_val,
            includes: none_val,
            defines: none_val,
        });

        // Create compilation outputs
        // Return lists of object files - these support len() which is needed by rules_cc
        let objects_list = heap.alloc(object_files.clone());
        let pic_objects_list = heap.alloc(pic_object_files.clone());
        let compilation_outputs = heap.alloc(CompilationOutputsGen {
            objects: objects_list,
            pic_objects: pic_objects_list,
        });

        // Return tuple of (compilation_context, compilation_outputs)
        Ok(heap.alloc((compilation_context, compilation_outputs)))
    }

    /// Gets the tool path for a given action.
    #[allow(unused_variables)]
    fn get_tool_for_action<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(cc_common): Implement proper tool lookup from feature configuration
        // For now, return placeholder tool names
        let tool = match action_name {
            "c-compile" | "c++-compile" => "/usr/bin/gcc",
            "c++-link-executable" | "c++-link-dynamic-library" => "/usr/bin/gcc",
            "c++-link-static-library" => "/usr/bin/ar",
            "strip" => "/usr/bin/strip",
            "objcopy" => "/usr/bin/objcopy",
            _ => "/usr/bin/gcc",
        };
        Ok(tool.to_owned())
    }

    /// Gets execution requirements for a given action.
    #[allow(unused_variables)]
    fn get_execution_requirements<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper execution requirements
        let map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        Ok(eval.heap().alloc(Dict::new(map)))
    }

    /// Checks if an action is enabled in the feature configuration.
    #[allow(unused_variables)]
    fn action_is_enabled<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // TODO(cc_common): Check against feature configuration
        // For now, all actions are considered enabled
        Ok(true)
    }

    /// Gets the command line for an action (memory inefficient version).
    #[allow(unused_variables)]
    fn get_memory_inefficient_command_line<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Generate actual command line from feature config
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    /// Gets environment variables for an action.
    #[allow(unused_variables)]
    fn get_environment_variables<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Extract env vars from feature configuration
        let map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        Ok(eval.heap().alloc(Dict::new(map)))
    }

    /// Creates empty toolchain variables.
    fn empty_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(CcToolchainVariablesGen {
            vars: Value::new_none(),
        }))
    }

    /// Gets legacy CC_FLAGS make variable value.
    #[allow(unused_variables)]
    fn legacy_cc_flags_make_variable_do_not_use<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(cc_common): Extract from toolchain
        Ok(String::new())
    }

    /// Checks if experimental cc_shared_library is enabled.
    fn check_experimental_cc_shared_library(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Checks if objc_library transition is disabled.
    fn incompatible_disable_objc_library_transition(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Checks if Go exec groups should be added to binary rules.
    fn add_go_exec_groups_to_binary_rules(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Checks if implementation_deps is allowed by allowlist.
    #[allow(unused_variables)]
    fn implementation_deps_allowed_by_allowlist<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Creates a compilation action (allowlisted).
    fn create_compile_action<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement create_compile_action
        Ok(NoneType)
    }

    /// Creates a linker input.
    #[allow(unused_variables)]
    fn create_linker_input<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] owner: Value<'v>,
        #[starlark(require = named, default = NoneType)] libraries: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_link_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] linkstamps: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(LinkerInputStubGen { owner, libraries }))
    }

    /// Creates a linking context.
    #[allow(unused_variables)]
    fn create_linking_context<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] linker_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] owner: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(LinkingContextWithInputsGen { linker_inputs }))
    }

    /// Checks if a feature is enabled in the feature configuration.
    #[allow(unused_variables)]
    fn is_enabled<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] feature_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // TODO(cc_common): Check against actual feature configuration
        // For now, return true for common features
        let enabled = matches!(
            feature_name,
            "supports_dynamic_linker"
                | "supports_interface_shared_libraries"
                | "pic"
                | "targets_windows"
                | "static_link_cpp_runtimes"
        );
        Ok(enabled)
    }

    /// Creates compilation outputs.
    #[allow(unused_variables)]
    fn create_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] lto_compilation_context: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CompilationOutputsGen {
            objects,
            pic_objects,
        }))
    }

    /// Merges compilation outputs.
    #[allow(unused_variables)]
    fn merge_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] compilation_outputs: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // For now, just return a new empty compilation outputs
        // TODO(cc_common): Actually merge the compilation outputs
        let none_val = Value::new_none();
        Ok(heap.alloc(CompilationOutputsGen {
            objects: none_val,
            pic_objects: none_val,
        }))
    }

    /// Creates a linking context from compilation outputs.
    ///
    /// Returns a tuple of (linking_context, linking_outputs).
    #[allow(unused_variables)]
    fn create_linking_context_from_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] compilation_outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_link_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] linking_contexts: Value<'v>,
        #[starlark(require = named, default = NoneType)] language: Value<'v>,
        #[starlark(require = named, default = false)] disallow_static_libraries: bool,
        #[starlark(require = named, default = false)] disallow_dynamic_library: bool,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] grep_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] stamp: Value<'v>,
        #[starlark(require = named, default = NoneType)] linked_dll_name_suffix: Value<'v>,
        #[starlark(require = named, default = NoneType)] win_def_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] test_only_target: Value<'v>,
        #[starlark(require = named, default = false)] alwayslink: bool,
        #[starlark(require = named, default = NoneType)] variables_extension: Value<'v>,
        #[starlark(require = named, default = NoneType)] main_output: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Create library_to_link (can be None for empty compilation outputs)
        let library_to_link = if compilation_outputs.is_none() {
            Value::new_none()
        } else {
            // Create a stub library_to_link
            heap.alloc(LibraryToLinkGen {
                static_library: Value::new_none(),
                pic_static_library: Value::new_none(),
                dynamic_library: Value::new_none(),
                interface_library: Value::new_none(),
                alwayslink,
            })
        };

        // Create linking outputs
        let linking_outputs = heap.alloc(CcLinkingOutputsGen { library_to_link });

        // Create linker_inputs depset
        // TODO(cc_common): Properly create depset with linker inputs from library_to_link
        // For now, use empty depset - proper depset creation requires FrozenValue
        let linker_inputs = heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty());

        // Create linking context
        let linking_context = heap.alloc(LinkingContextWithInputsGen { linker_inputs });

        // Return tuple
        Ok(heap.alloc((linking_context, linking_outputs)))
    }

    /// Merges multiple linking contexts into one.
    #[allow(unused_variables)]
    fn merge_linking_contexts<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] linking_contexts: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // For now, return an empty linking context
        // TODO(cc_common): Properly merge linker inputs from all contexts
        // This requires handling depset merging which needs FrozenValue support
        let linker_inputs = heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty());

        Ok(heap.alloc(LinkingContextWithInputsGen { linker_inputs }))
    }

    /// Creates a library_to_link struct.
    #[allow(unused_variables)]
    fn create_library_to_link<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] static_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_static_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] dynamic_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] interface_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] objects: Value<'v>,
        #[starlark(require = named, default = false)] alwayslink: bool,
        #[starlark(require = named, default = NoneType)] dynamic_library_symlink_path: Value<'v>,
        #[starlark(require = named, default = NoneType)] interface_library_symlink_path: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(LibraryToLinkGen {
            static_library,
            pic_static_library,
            dynamic_library,
            interface_library,
            alwayslink,
        }))
    }

    /// Returns tool execution requirements for an action.
    ///
    /// Returns a list of execution requirements (strings like "requires-network")
    /// that should be added to actions using the specified tool.
    #[allow(unused_variables)]
    fn get_tool_requirement_for_action<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty list - no special execution requirements
        Ok(eval.heap().alloc(Vec::<String>::new()))
    }

    /// Creates link variables for use with get_memory_inefficient_command_line.
    ///
    /// Used by rules_rust to get linker command line from cc toolchain.
    #[allow(unused_variables)]
    fn create_link_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = false)] is_linking_dynamic_library: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        runtime_library_search_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        user_link_flags: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return a CcToolchainVariables stub - the actual link flags are
        // computed by get_memory_inefficient_command_line
        Ok(eval.heap().alloc(CcToolchainVariablesGen {
            vars: Value::new_none(),
        }))
    }

    /// Merges multiple CcInfo providers into a single CcInfo.
    ///
    /// Used to combine CcInfo from multiple dependencies.
    #[allow(unused_variables)]
    fn merge_cc_infos<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] cc_infos: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_cc_infos: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<CcInfoInstanceStub> {
        // Return a stub CcInfo - merging is a no-op for now
        Ok(CcInfoInstanceStub)
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
    objects: V,
    pic_objects: V,
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
    static_library: V,
    pic_static_library: V,
    dynamic_library: V,
    interface_library: V,
    alwayslink: bool,
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
                | "alwayslink"
        )
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "static_library" => Some(self.static_library.to_value()),
            "pic_static_library" => Some(self.pic_static_library.to_value()),
            "dynamic_library" => Some(self.dynamic_library.to_value()),
            "interface_library" => Some(self.interface_library.to_value()),
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
    library_to_link: V,
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
        matches!(attribute, "library_to_link")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "library_to_link" => Some(self.library_to_link.to_value()),
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

impl Display for CcToolchainInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcToolchainInfo>")
    }
}

starlark_simple_value!(CcToolchainInfoProvider);

#[starlark_value(type = "CcToolchainInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainInfoProvider {}

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
        // Return a stub CcInfo instance
        let _kwargs = args.names_map()?;
        let heap = eval.heap();
        Ok(heap.alloc(CcInfoInstanceStub))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// A stub CcInfo instance (returned when CcInfo(...) is called).
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
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "compilation_context" | "linking_context")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use crate::interpreter::rule_defs::context::CompilationContextStub;
        use crate::interpreter::rule_defs::context::LinkingContextStub;
        match attribute {
            "compilation_context" => Some(heap.alloc(CompilationContextStub)),
            "linking_context" => Some(heap.alloc(LinkingContextStub)),
            _ => None,
        }
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

// ============================================================================
// DebugPackageInfo - Debug information provider
// ============================================================================

/// DebugPackageInfo provider for debug/symbol information.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct DebugPackageInfoProvider;

impl Display for DebugPackageInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider DebugPackageInfo>")
    }
}

starlark_simple_value!(DebugPackageInfoProvider);

#[starlark_value(type = "DebugPackageInfo")]
impl<'v> StarlarkValue<'v> for DebugPackageInfoProvider {}

// ============================================================================
// CcSharedLibraryInfo - Shared library information provider
// ============================================================================

/// CcSharedLibraryInfo provider for shared library information.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcSharedLibraryInfoProvider;

impl Display for CcSharedLibraryInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcSharedLibraryInfo>")
    }
}

starlark_simple_value!(CcSharedLibraryInfoProvider);

#[starlark_value(type = "CcSharedLibraryInfo")]
impl<'v> StarlarkValue<'v> for CcSharedLibraryInfoProvider {}

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
        // OutputGroupInfo doesn't have fixed fields - it has dynamic output groups
        // Return empty for now since the groups are stored in a dict
        vec![]
    }
}

#[starlark::values::starlark_value(type = "OutputGroupInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for OutputGroupInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        // Check if attribute exists in groups dict by trying to iterate
        if let Ok(iter) = self.groups.to_value().iterate(heap) {
            for key in iter {
                if key.unpack_str() == Some(attribute) {
                    return true;
                }
            }
        }
        false
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        // Get attribute from groups dict using at2
        let key = heap.alloc_str(attribute);
        self.groups.to_value().at(key.to_value(), heap).ok()
    }

    // Support 'in' operator - note: cannot use heap here, so we iterate the dict keys
    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        // Always return true for now - proper implementation would need heap
        // This is used for `"key" in output_group_info` checks
        Ok(true)
    }

    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Index into groups dict
        self.groups.to_value().at(index, heap)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

impl Display for CcToolchainConfigInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcToolchainConfigInfo>")
    }
}

starlark_simple_value!(CcToolchainConfigInfoProvider);

#[starlark_value(type = "CcToolchainConfigInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainConfigInfoProvider {}

// ============================================================================
// Registration
// ============================================================================

/// Register the cc_common global and related providers.
///
/// Note: Per Bazel's CcRules.java, some providers are set to None because
/// they are defined in Starlark by rules_cc.
#[starlark_module]
pub fn register_cc_common(globals: &mut GlobalsBuilder) {
    /// The cc_common module provides C/C++ compilation support.
    const cc_common: CcCommonModule = CcCommonModule;

    /// CcInfo provider for C++ compilation/linking information.
    const CcInfo: CcInfoProvider = CcInfoProvider;

    /// CcToolchainInfo provider for C++ toolchain information.
    const CcToolchainInfo: CcToolchainInfoProvider = CcToolchainInfoProvider;

    /// CcToolchainConfigInfo provider for toolchain configuration.
    /// Used by cc_common.create_cc_toolchain_config_info().
    const CcToolchainConfigInfo: CcToolchainConfigInfoProvider = CcToolchainConfigInfoProvider;

    /// DebugPackageInfo - None placeholder. Actual provider defined in rules_cc Starlark.
    const DebugPackageInfo: NoneType = NoneType;

    /// CcSharedLibraryInfo - None placeholder. Actual provider defined in rules_cc Starlark.
    const CcSharedLibraryInfo: NoneType = NoneType;

    /// OutputGroupInfo - provider for grouping outputs.
    /// This is callable to create instances.
    const OutputGroupInfo: OutputGroupInfoProvider = OutputGroupInfoProvider;

    /// PackageSpecificationInfo - None placeholder.
    /// This is a Bazel built-in provider for package visibility/allowlisting.
    /// Used by cc_toolchain.bzl for visibility_public_presubmit attribute.
    const PackageSpecificationInfo: NoneType = NoneType;

    /// RunEnvironmentInfo - Callable stub.
    /// This is a Bazel built-in provider for specifying environment variables
    /// that should be set when running binaries or tests.
    /// Returns None for now; proper implementation would return a provider instance.
    /// TODO(bazel): Implement proper RunEnvironmentInfo provider.
    #[starlark(speculative_exec_safe)]
    fn RunEnvironmentInfo<'v>(
        #[starlark(require = named)] environment: Option<Value<'v>>,
        #[starlark(require = named)] inherited_environment: Option<Value<'v>>,
    ) -> starlark::Result<NoneType> {
        let _unused = (environment, inherited_environment);
        Ok(NoneType)
    }

    /// testing module constant for Bazel-compatible testing utilities.
    /// Currently a stub that provides TestEnvironment.
    const testing: TestingModule = TestingModule;
}

// ============================================================================
// TestingModule - Bazel's testing module
// ============================================================================

/// Stub for the testing module.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct TestingModule;

impl Display for TestingModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<module: testing>")
    }
}

starlark_simple_value!(TestingModule);

#[starlark_value(type = "testing")]
impl<'v> StarlarkValue<'v> for TestingModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(testing_methods)
    }
}

#[starlark_module]
fn testing_methods(builder: &mut MethodsBuilder) {
    /// TestEnvironment provider for specifying test environment variables.
    /// Returns None for now - proper implementation would return a TestEnvironment provider.
    fn TestEnvironment<'v>(
        this: &TestingModule,
        #[starlark(require = named)] environment: Option<Value<'v>>,
        #[starlark(require = named)] inherited_environment: Option<Value<'v>>,
    ) -> starlark::Result<NoneType> {
        let _unused = (this, environment, inherited_environment);
        Ok(NoneType)
    }

    /// ExecutionInfo provider for specifying execution requirements.
    /// This is a stub that returns a sentinel value so version checks
    /// like `testing.ExecutionInfo == testing.ExecutionInfo` evaluate to True.
    #[starlark(attribute)]
    fn ExecutionInfo<'v>(this: &TestingModule) -> starlark::Result<&'static str> {
        let _unused = this;
        Ok("ExecutionInfo")
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
    owner: V,
    libraries: V,
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
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "additional_inputs" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
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
    linker_inputs: V,
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
