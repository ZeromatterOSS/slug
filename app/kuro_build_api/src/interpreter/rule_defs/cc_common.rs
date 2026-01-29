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

use allocative::Allocative;
use starlark::collections::SmallMap;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::dict::Dict;
use starlark::values::list::AllocList;
use starlark::values::none::NoneType;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;
use std::fmt;
use std::fmt::Display;

// ============================================================================
// CcToolchainVariables - Variables for C++ toolchain configuration
// ============================================================================

/// CcToolchainVariables holds build variables for C++ toolchain configuration.
///
/// Used by cc_common functions to pass configuration to compile/link actions.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct CcToolchainVariables {
    // Internal storage for variables - currently a stub
    // TODO(cc_common): Implement full variable storage and lookup
    _empty: bool,
}

impl Display for CcToolchainVariables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcToolchainVariables()")
    }
}

starlark_simple_value!(CcToolchainVariables);

#[starlark_value(type = "CcToolchainVariables")]
impl<'v> StarlarkValue<'v> for CcToolchainVariables {}

// ============================================================================
// CcCommonInternal - Internal API returned by internal_DO_NOT_USE()
// ============================================================================

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
    /// TODO(cc_common): Implement actual compile action creation.
    fn create_cc_compile_action<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement create_cc_compile_action
        // This should create a compilation action for C/C++ source files
        Ok(NoneType)
    }

    /// Gets the artifact name for a given category.
    ///
    /// Categories include: "object_file", "pic_object_file", "executable", etc.
    fn get_artifact_name_for_category<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _cc_toolchain: Value<'v>,
        #[starlark(require = named)] category: &str,
        #[starlark(require = named, default = "")] output_name: &str,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(cc_common): Implement proper artifact naming based on toolchain
        // For now, return basic naming conventions
        let name = if output_name.is_empty() {
            "output"
        } else {
            output_name
        };
        let result = match category {
            "object_file" => format!("{}.o", name),
            "pic_object_file" => format!("{}.pic.o", name),
            "executable" => name.to_owned(),
            "static_library" => format!("lib{}.a", name),
            "dynamic_library" => format!("lib{}.so", name),
            "interface_library" => format!("lib{}.so", name),
            _ => format!("{}.{}", name, category),
        };
        Ok(result)
    }

    /// Combines toolchain variables from multiple sources.
    fn combine_cc_toolchain_variables<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] _parent: Value<'v>,
        #[starlark(require = named, default = NoneType)] _child: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<CcToolchainVariables> {
        // TODO(cc_common): Implement proper variable combination
        Ok(CcToolchainVariables { _empty: false })
    }

    /// Gets the rule context from an actions object.
    ///
    /// This is a workaround used by rules_cc to access ctx from actions.
    fn actions2ctx_cheat<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _actions: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement actions2ctx_cheat
        // This should return the ctx object from an actions object
        Ok(NoneType)
    }

    /// Creates CcToolchainVariables from a dictionary.
    fn cc_toolchain_variables<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] _vars: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<CcToolchainVariables> {
        // TODO(cc_common): Implement proper variable creation from dict
        Ok(CcToolchainVariables { _empty: false })
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

    /// Creates a tree artifact compile action template.
    fn create_cc_compile_action_template<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement tree artifact compile template
        Ok(NoneType)
    }

    /// Wraps link actions for platform compatibility.
    fn wrap_link_actions<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _actions: Value<'v>,
        #[starlark(require = named)] _linking_outputs: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement link action wrapping
        Ok(NoneType)
    }

    /// Gets link arguments from a linking context.
    fn get_link_args<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _linking_context: Value<'v>,
        #[starlark(require = named, default = false)] _expand_to_linker_flags: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper link args extraction
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    /// Declares a compile output file.
    fn declare_compile_output_file<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _actions: Value<'v>,
        #[starlark(require = named)] _cc_toolchain: Value<'v>,
        #[starlark(require = named)] _source_file: Value<'v>,
        #[starlark(require = named, default = "")] _output_name: &str,
        #[starlark(require = named, default = false)] _pic: bool,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement compile output declaration
        Ok(NoneType)
    }

    /// Declares an auxiliary output file (dwo, gcno, etc.).
    fn declare_other_output_file<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _actions: Value<'v>,
        #[starlark(require = named)] _cc_toolchain: Value<'v>,
        #[starlark(require = named)] _source_file: Value<'v>,
        #[starlark(require = named, default = "")] _extension: &str,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
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
    fn compute_output_name_prefix_dir<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _output_name: &str,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(cc_common): Implement proper output prefix computation
        Ok(String::new())
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
    fn per_file_copts<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _ctx: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement per-file copts
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    /// Checks access to private API (allowlist enforcement).
    fn check_private_api<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] _allowlist: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // Always allow for now
        Ok(true)
    }

    /// Creates a HeaderInfo struct.
    fn create_header_info<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] _headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] _modular_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] _textual_headers: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper HeaderInfo struct
        // Return an empty dict for now
        let map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        Ok(eval.heap().alloc(Dict::new(map)))
    }

    /// Creates a HeaderInfo struct with dependency tracking.
    fn create_header_info_with_deps<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] _headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] _modular_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] _textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] _deps: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper HeaderInfo with deps
        let map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        Ok(eval.heap().alloc(Dict::new(map)))
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
    fn internal_DO_NOT_USE(
        this: &CcCommonModule,
    ) -> starlark::Result<CcCommonInternal> {
        let _ = this;
        Ok(CcCommonInternal)
    }

    /// Gets the tool path for a given action.
    fn get_tool_for_action<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] _feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
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
    fn get_execution_requirements<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] _feature_configuration: Value<'v>,
        #[starlark(require = named)] _action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper execution requirements
        let map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        Ok(eval.heap().alloc(Dict::new(map)))
    }

    /// Checks if an action is enabled in the feature configuration.
    fn action_is_enabled<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] _feature_configuration: Value<'v>,
        #[starlark(require = named)] _action_name: &str,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // TODO(cc_common): Check against feature configuration
        // For now, all actions are considered enabled
        Ok(true)
    }

    /// Gets the command line for an action (memory inefficient version).
    fn get_memory_inefficient_command_line<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] _feature_configuration: Value<'v>,
        #[starlark(require = named)] _action_name: &str,
        #[starlark(require = named)] _variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Generate actual command line from feature config
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    /// Gets environment variables for an action.
    fn get_environment_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] _feature_configuration: Value<'v>,
        #[starlark(require = named)] _action_name: &str,
        #[starlark(require = named)] _variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Extract env vars from feature configuration
        let map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        Ok(eval.heap().alloc(Dict::new(map)))
    }

    /// Creates empty toolchain variables.
    fn empty_variables(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<CcToolchainVariables> {
        Ok(CcToolchainVariables { _empty: true })
    }

    /// Gets legacy CC_FLAGS make variable value.
    fn legacy_cc_flags_make_variable_do_not_use<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] _cc_toolchain: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
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
    fn implementation_deps_allowed_by_allowlist<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] _ctx: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
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

/// CcInfo provider stub - contains C++ compilation and linking information.
///
/// In Bazel 9.0+, CcInfo is actually defined in pure Starlark in rules_cc
/// (cc/private/cc_info.bzl). This native stub exists for compatibility with
/// code that references the native CcInfo before rules_cc is loaded.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcInfoProvider;

impl Display for CcInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcInfo>")
    }
}

starlark_simple_value!(CcInfoProvider);

#[starlark_value(type = "CcInfo")]
impl<'v> StarlarkValue<'v> for CcInfoProvider {}

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

impl Display for OutputGroupInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider OutputGroupInfo>")
    }
}

starlark_simple_value!(OutputGroupInfoProvider);

#[starlark_value(type = "OutputGroupInfo")]
impl<'v> StarlarkValue<'v> for OutputGroupInfoProvider {}

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

    /// CcInfo - None placeholder. Actual provider defined in rules_cc Starlark.
    const CcInfo: NoneType = NoneType;

    /// CcToolchainInfo provider for C++ toolchain information.
    const CcToolchainInfo: CcToolchainInfoProvider = CcToolchainInfoProvider;

    /// CcToolchainConfigInfo provider for toolchain configuration.
    /// Used by cc_common.create_cc_toolchain_config_info().
    const CcToolchainConfigInfo: CcToolchainConfigInfoProvider = CcToolchainConfigInfoProvider;

    /// DebugPackageInfo - None placeholder. Actual provider defined in rules_cc Starlark.
    const DebugPackageInfo: NoneType = NoneType;

    /// CcSharedLibraryInfo - None placeholder. Actual provider defined in rules_cc Starlark.
    const CcSharedLibraryInfo: NoneType = NoneType;

    /// OutputGroupInfo - None placeholder for now.
    /// TODO(bazel): Implement proper OutputGroupInfo provider.
    const OutputGroupInfo: NoneType = NoneType;

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
}
