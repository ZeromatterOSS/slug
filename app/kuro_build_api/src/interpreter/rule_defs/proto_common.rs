/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible proto_common module and ProtoInfo provider.
//!
//! This provides an implementation of Bazel's ProtoInfo provider and proto_common
//! built-in module that protobuf rules require for protocol buffer compilation support.
//!
//! ## Architecture
//!
//! The proto_common module has two types of methods:
//!
//! ### Action Primitives (require native implementation)
//! - `compile()` - Creates proto compilation actions
//!
//! ### Stub Methods (can be Starlark)
//! These methods return hardcoded values. Starlark helper implementations are
//! available in `@bazel_tools//tools/build_defs/proto:proto_common.bzl`:
//! - `proto_path_flag()` - Returns "--proto_path="
//! - `descriptor_set_flag()` - Returns "--descriptor_set_out="
//! - `get_tool_path()` - Returns "/usr/bin/protoc"
//! - `has_plugin()` - Returns False
//! - `experimental_use_proto_source_order()` - Returns False
//!
//! ## Symbols
//!
//! The protobuf rules (specifically `protobuf//bazel/private/native.bzl`) expect
//! these to be available as global symbols:
//! - `ProtoInfo` - Provider for proto compilation information (None placeholder)
//! - `proto_common_do_not_use` - Internal proto compilation utilities
//!
//! Reference: https://bazel.build/rules/lib/ProtoInfo

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
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

// ============================================================================
// ProtoInfo Provider - Encapsulates information provided by proto_library
// ============================================================================

/// ProtoInfo provider for protocol buffer compilation information.
///
/// This provider encapsulates information about .proto files and their compilation,
/// including source files, descriptor sets, and source roots.
///
/// Note: This is a stub implementation that allows protobuf rules to load.
/// The actual ProtoInfo provider is defined in Starlark by the protobuf rules
/// once they can load (using the `provider()` function).
///
/// Reference: https://bazel.build/rules/lib/ProtoInfo
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct ProtoInfoProvider;

impl Display for ProtoInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider ProtoInfo>")
    }
}

starlark_simple_value!(ProtoInfoProvider);

#[starlark_value(type = "ProtoInfo")]
impl<'v> StarlarkValue<'v> for ProtoInfoProvider {}

// ============================================================================
// proto_common_do_not_use - Internal proto compilation utilities
// ============================================================================

/// The proto_common_do_not_use module provides proto compilation utilities.
///
/// This is an internal API used by protobuf rules. The "do_not_use" suffix
/// indicates this is not a stable public API.
///
/// Reference: protobuf//bazel/private/native.bzl
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ProtoCommonModule;

impl Display for ProtoCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proto_common")
    }
}

starlark_simple_value!(ProtoCommonModule);

#[starlark_value(type = "proto_common")]
impl<'v> StarlarkValue<'v> for ProtoCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(proto_common_module_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION"
                | "ProtoInfo"
                | "compile"
                | "proto_path_flag"
                | "descriptor_set_flag"
        )
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            // This flag controls whether proto toolchain resolution is enabled.
            // Set to false because Kuro doesn't implement Bazel-style toolchain resolution.
            // This causes protobuf rules to use the legacy codepath with _proto_compiler attr.
            "INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION" => Some(Value::new_bool(false)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "INCOMPATIBLE_ENABLE_PROTO_TOOLCHAIN_RESOLUTION".to_owned(),
            "ProtoInfo".to_owned(),
            "compile".to_owned(),
            "proto_path_flag".to_owned(),
            "descriptor_set_flag".to_owned(),
        ]
    }
}

/// Methods on the proto_common module.
///
/// Methods are organized into two categories:
/// 1. ACTION PRIMITIVES - Must be implemented natively (compile)
/// 2. STUBS - Return hardcoded values, can be Starlark
///    (see @bazel_tools//tools/build_defs/proto:proto_common.bzl)
#[starlark_module]
fn proto_common_module_methods(builder: &mut MethodsBuilder) {
    // =========================================================================
    // ACTION PRIMITIVES - These methods require native implementation
    // =========================================================================

    /// Compiles proto files using the proto toolchain.
    ///
    /// ACTION PRIMITIVE: Requires native implementation.
    /// TODO(proto_common): Implement actual proto compilation.
    fn compile<'v>(
        #[starlark(this)] _this: &ProtoCommonModule,
        #[starlark(require = named)] _actions: Value<'v>,
        #[starlark(require = named)] _proto_info: Value<'v>,
        #[starlark(require = named, default = NoneOr::None)] _proto_lang_toolchain_info: NoneOr<
            Value<'v>,
        >,
        #[starlark(require = named, default = NoneOr::None)] _generated_files: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] _plugin_output: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] _additional_args: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] _additional_inputs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] _additional_tools: NoneOr<Value<'v>>,
        #[starlark(require = named, default = "")] _resource_set: &str,
        #[starlark(require = named, default = false)] _experimental_progress_message: bool,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(proto_common): Implement actual proto compilation
        Ok(NoneType)
    }

    // =========================================================================
    // STUB METHODS - These can be moved to Starlark
    // =========================================================================

    /// Returns the proto path flag (e.g., "--proto_path=").
    ///
    /// STUB: Can be Starlark. See proto_common.bzl proto_path_flag_helper().
    fn proto_path_flag<'v>(
        #[starlark(this)] _this: &ProtoCommonModule,
        #[starlark(require = named)] _proto_lang_toolchain_info: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(proto_common): Extract from toolchain
        Ok("--proto_path=".to_owned())
    }

    /// Returns the descriptor set output flag.
    ///
    /// STUB: Can be Starlark. See proto_common.bzl descriptor_set_flag_helper().
    fn descriptor_set_flag<'v>(
        #[starlark(this)] _this: &ProtoCommonModule,
        #[starlark(require = named)] _proto_lang_toolchain_info: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(proto_common): Extract from toolchain
        Ok("--descriptor_set_out=".to_owned())
    }

    /// Checks if experimental_use_proto_source_order is enabled.
    ///
    /// STUB: Can be Starlark. See proto_common.bzl experimental_use_proto_source_order_helper().
    fn experimental_use_proto_source_order(
        #[starlark(this)] _this: &ProtoCommonModule,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Gets a tool path from the proto toolchain.
    ///
    /// STUB: Can be Starlark. See proto_common.bzl get_tool_path_helper().
    fn get_tool_path<'v>(
        #[starlark(this)] _this: &ProtoCommonModule,
        #[starlark(require = named)] _proto_lang_toolchain_info: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(proto_common): Extract from toolchain
        let path = if cfg!(windows) { "protoc.exe" } else { "/usr/bin/protoc" };
        Ok(path.to_owned())
    }

    /// Checks if a proto toolchain has a plugin.
    ///
    /// STUB: Can be Starlark. See proto_common.bzl has_plugin_helper().
    fn has_plugin<'v>(
        #[starlark(this)] _this: &ProtoCommonModule,
        #[starlark(require = named)] _proto_lang_toolchain_info: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // TODO(proto_common): Check toolchain for plugin
        Ok(false)
    }
}

// ============================================================================
// ProtoLangToolchainInfo - Provider for language-specific proto toolchains
// ============================================================================

/// ProtoLangToolchainInfo provider for language-specific proto compilation.
///
/// This provider carries configuration for compiling protos to a specific
/// language (e.g., Java, Python, C++).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ProtoLangToolchainInfoProvider;

impl Display for ProtoLangToolchainInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider ProtoLangToolchainInfo>")
    }
}

starlark_simple_value!(ProtoLangToolchainInfoProvider);

#[starlark_value(type = "ProtoLangToolchainInfo")]
impl<'v> StarlarkValue<'v> for ProtoLangToolchainInfoProvider {}

// ============================================================================
// ProtoModule - Bazel's proto module (proto.encode_text, etc.)
// ============================================================================

/// The proto module provides protocol buffer utilities.
///
/// In Bazel, `proto.encode_text(x)` converts a struct/dict to text proto format.
///
/// Reference: https://bazel.build/rules/lib/toplevel/proto
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ProtoModule;

impl Display for ProtoModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proto")
    }
}

starlark_simple_value!(ProtoModule);

#[starlark_value(type = "proto")]
impl<'v> StarlarkValue<'v> for ProtoModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(proto_module_methods)
    }
}

#[starlark_module]
fn proto_module_methods(builder: &mut MethodsBuilder) {
    /// Encodes a value to text proto format.
    ///
    /// Converts a Starlark struct or dict to a textproto string representation.
    ///
    /// Reference: https://bazel.build/rules/lib/toplevel/proto#encode_text
    fn encode_text<'v>(
        #[starlark(this)] _this: &ProtoModule,
        #[starlark(require = pos)] x: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // Simple textproto encoding: just use display for now
        // A full implementation would recursively format struct fields
        Ok(format!("{}", x))
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register the proto globals (ProtoInfo, proto_common_do_not_use, proto).
///
/// Note on provider registrations:
/// ProtoInfo is deprecated as a native global in Bazel 8+.
/// The actual provider is defined in Starlark by the protobuf rules.
///
/// Reference: thoughts/shared/plans/kuro-bazel-subplans/02-bzlmod.md lines 86-128
#[starlark_module]
pub fn register_proto_common(globals: &mut GlobalsBuilder) {
    /// ProtoInfo - None placeholder. Deprecated in Bazel 8+.
    /// Actual provider defined in protobuf rules Starlark.
    const ProtoInfo: NoneType = NoneType;

    /// Internal proto compilation utilities (proto_common).
    /// This is exposed as proto_common_do_not_use for backward compatibility.
    const proto_common_do_not_use: ProtoCommonModule = ProtoCommonModule;

    /// ProtoLangToolchainInfo provider for language-specific proto toolchains.
    const ProtoLangToolchainInfo: ProtoLangToolchainInfoProvider = ProtoLangToolchainInfoProvider;

    /// The proto module for protocol buffer utilities.
    const proto: ProtoModule = ProtoModule;
}
