/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible java_common module and JavaInfo provider.
//!
//! This provides an implementation of Bazel's java_common built-in module
//! that Java rules (rules_java) require for Java compilation support.
//!
//! ## Symbols
//!
//! - `java_common` - Module with Java compilation utilities
//! - `JavaInfo` - Callable provider for Java compilation information
//! - `JavaPluginInfo` - Callable provider for Java annotation processors
//!
//! Reference: https://bazel.build/rules/lib/toplevel/java_common

use std::fmt;
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
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Demand;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::dict::AllocDict;
use starlark::values::list::AllocList;
use starlark::values::none::NoneOr;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::depset::Depset;
use crate::interpreter::rule_defs::py_common::NativeProviderInstance;
use crate::interpreter::rule_defs::py_common::create_native_provider_instance;

/// Provider index for JavaInfo in the NativeProviderInstance dispatch table.
pub const JAVA_INFO_IDX: u32 = 2;
/// Provider index for JavaPluginInfo in the NativeProviderInstance dispatch table.
pub const JAVA_PLUGIN_INFO_IDX: u32 = 3;
/// Provider index for JavaRuntimeInfo in the NativeProviderInstance dispatch table.
pub const JAVA_RUNTIME_INFO_IDX: u32 = 4;
/// Provider index for JavaToolchainInfo in the NativeProviderInstance dispatch table.
pub const JAVA_TOOLCHAIN_INFO_IDX: u32 = 5;

// ============================================================================
// JavaCommonModule - The main java_common namespace
// ============================================================================

/// The java_common module provides Java compilation utilities.
///
/// Reference: https://bazel.build/rules/lib/toplevel/java_common
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaCommonModule;

impl Display for JavaCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "java_common")
    }
}

starlark_simple_value!(JavaCommonModule);

#[starlark_value(type = "java_common")]
impl<'v> StarlarkValue<'v> for JavaCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(java_common_module_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "JavaRuntimeInfo"
                | "JavaToolchainInfo"
                | "provider"
                | "INCOMPATIBLE_ENABLE_JAVA_TOOLCHAIN_RESOLUTION"
                | "internal_DO_NOT_USE"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "INCOMPATIBLE_ENABLE_JAVA_TOOLCHAIN_RESOLUTION" => Some(Value::new_bool(true)),
            "JavaRuntimeInfo" => Some(heap.alloc(JavaRuntimeInfoProvider)),
            "JavaToolchainInfo" => Some(heap.alloc(JavaToolchainInfoProvider)),
            "provider" => Some(heap.alloc(JavaInfoProvider)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "JavaRuntimeInfo".to_owned(),
            "JavaToolchainInfo".to_owned(),
            "provider".to_owned(),
            "INCOMPATIBLE_ENABLE_JAVA_TOOLCHAIN_RESOLUTION".to_owned(),
        ]
    }
}

/// Methods on the java_common module.
#[starlark_module]
fn java_common_module_methods(builder: &mut MethodsBuilder) {
    /// Compiles Java sources and returns a JavaInfo provider.
    ///
    /// This is the primary Java compilation method used by rules_java.
    /// Returns a JavaInfo instance with compilation outputs.
    #[allow(unused_variables)]
    fn compile<'v>(
        #[starlark(this)] _this: &JavaCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named, default = NoneOr::None)] source_jars: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] source_files: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] output: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] output_source_jar: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] javac_opts: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] deps: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] runtime_deps: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] exports: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] plugins: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] exported_plugins: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] native_libraries: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] annotation_processor_additional_inputs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)]
        annotation_processor_additional_outputs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] strict_deps: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] java_toolchain: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] neverlink: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] enable_compile_jar_action: NoneOr<
            bool,
        >,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Build a JavaInfo instance with the compile outputs.
        // In Bazel, compile() returns a JavaInfo with:
        // - compile_jar: the output jar
        // - source_jar: the output source jar
        // - deps: transitive compile-time deps
        // - runtime_deps: transitive runtime deps
        // - transitive_compile_time_jars: depset of jars needed at compile time
        // - transitive_runtime_jars: depset of jars needed at runtime
        let output_val = output.into_option().unwrap_or(Value::new_none());
        let source_jar_val = output_source_jar.into_option().unwrap_or(Value::new_none());
        let deps_val = deps
            .into_option()
            .unwrap_or_else(|| heap.alloc(AllocList::EMPTY));
        let runtime_deps_val = runtime_deps
            .into_option()
            .unwrap_or_else(|| heap.alloc(AllocList::EMPTY));
        let exports_val = exports
            .into_option()
            .unwrap_or_else(|| heap.alloc(AllocList::EMPTY));
        let plugins_val = plugins
            .into_option()
            .unwrap_or_else(|| heap.alloc(AllocList::EMPTY));
        let neverlink_val = Value::new_bool(neverlink.into_option().unwrap_or(false));

        // Create empty depsets for transitive jars (would be populated by real compilation)
        let empty_depset = heap.alloc(Depset::empty());

        let pairs: Vec<(Value<'v>, Value<'v>)> = vec![
            (heap.alloc_str("compile_jar").to_value(), output_val),
            (heap.alloc_str("source_jar").to_value(), source_jar_val),
            (heap.alloc_str("deps").to_value(), deps_val),
            (heap.alloc_str("runtime_deps").to_value(), runtime_deps_val),
            (heap.alloc_str("exports").to_value(), exports_val),
            (heap.alloc_str("plugins").to_value(), plugins_val),
            (heap.alloc_str("neverlink").to_value(), neverlink_val),
            (
                heap.alloc_str("transitive_compile_time_jars").to_value(),
                empty_depset,
            ),
            (
                heap.alloc_str("transitive_runtime_jars").to_value(),
                empty_depset,
            ),
            (heap.alloc_str("compile_jars").to_value(), empty_depset),
            (heap.alloc_str("full_compile_jars").to_value(), empty_depset),
            (heap.alloc_str("source_jars").to_value(), empty_depset),
            (
                heap.alloc_str("runtime_output_jars").to_value(),
                empty_depset,
            ),
            (
                heap.alloc_str("transitive_source_jars").to_value(),
                empty_depset,
            ),
            (
                heap.alloc_str("transitive_native_libraries").to_value(),
                empty_depset,
            ),
            (heap.alloc_str("outputs").to_value(), empty_depset),
        ];
        let dict = heap.alloc(AllocDict(pairs));

        Ok(heap.alloc(NativeProviderInstance {
            values: dict,
            provider_idx: JAVA_INFO_IDX,
        }))
    }

    /// Merges multiple JavaInfo providers into one.
    #[allow(unused_variables)]
    fn merge<'v>(
        #[starlark(this)] _this: &JavaCommonModule,
        #[starlark(require = pos)] providers: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Collect all kwargs from the input JavaInfo providers and merge them.
        // For a proper implementation we'd merge depsets, but for now we create
        // a new JavaInfo with empty transitive fields.
        let empty = heap.alloc(AllocList::EMPTY);
        let pairs: Vec<(Value<'v>, Value<'v>)> = vec![
            (heap.alloc_str("compile_jar").to_value(), Value::new_none()),
            (heap.alloc_str("source_jar").to_value(), Value::new_none()),
            (heap.alloc_str("deps").to_value(), empty),
            (heap.alloc_str("runtime_deps").to_value(), empty),
            (heap.alloc_str("exports").to_value(), empty),
            (heap.alloc_str("plugins").to_value(), empty),
            (
                heap.alloc_str("neverlink").to_value(),
                Value::new_bool(false),
            ),
            (
                heap.alloc_str("transitive_compile_time_jars").to_value(),
                empty,
            ),
            (heap.alloc_str("transitive_runtime_jars").to_value(), empty),
            (heap.alloc_str("compile_jars").to_value(), empty),
            (heap.alloc_str("full_compile_jars").to_value(), empty),
            (heap.alloc_str("source_jars").to_value(), empty),
            (heap.alloc_str("runtime_output_jars").to_value(), empty),
            (heap.alloc_str("transitive_source_jars").to_value(), empty),
            (
                heap.alloc_str("transitive_native_libraries").to_value(),
                empty,
            ),
            (heap.alloc_str("outputs").to_value(), empty),
        ];
        let dict = heap.alloc(AllocDict(pairs));

        Ok(heap.alloc(NativeProviderInstance {
            values: dict,
            provider_idx: JAVA_INFO_IDX,
        }))
    }

    /// Returns a non-strict version of a JavaInfo.
    #[allow(unused_variables)]
    fn make_non_strict<'v>(
        #[starlark(this)] _this: &JavaCommonModule,
        #[starlark(require = pos)] java_info: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // In Bazel, make_non_strict returns a copy of the JavaInfo with
        // strict_deps turned off. For compatibility, just return the input.
        Ok(java_info)
    }

    /// Returns the default Java toolchain bootclasspath.
    fn boot_class_path<'v>(
        #[starlark(this)] _this: &JavaCommonModule,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    /// Internal API used by rules_java.
    /// Returns an object with google_legacy_api_enabled() and
    /// check_java_toolchain_is_declared_on_rule() methods.
    fn internal_DO_NOT_USE<'v>(
        #[starlark(this)] _this: &JavaCommonModule,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(JavaCommonInternal))
    }
}

/// Internal Java common API (returned by java_common.internal_DO_NOT_USE()).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
struct JavaCommonInternal;

impl Display for JavaCommonInternal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<java_common_internal>")
    }
}

starlark_simple_value!(JavaCommonInternal);

#[starlark_value(type = "java_common_internal")]
impl<'v> StarlarkValue<'v> for JavaCommonInternal {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(java_common_internal_methods)
    }
}

#[starlark_module]
fn java_common_internal_methods(builder: &mut MethodsBuilder) {
    fn google_legacy_api_enabled<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    fn incompatible_disable_non_executable_java_binary<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    fn incompatible_java_info_merge_runtime_module_flags<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    #[allow(unused_variables)]
    fn check_java_toolchain_is_declared_on_rule<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
        actions: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    #[allow(unused_variables)]
    fn check_provider_instances<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
        providers: Value<'v>,
        what: &str,
        provider_type: Value<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(Value::new_none())
    }

    #[allow(unused_variables)]
    fn expand_java_opts<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
        ctx: Value<'v>,
        attr: &str,
        #[starlark(require = named)] tokenize: bool,
        #[starlark(require = named, default = false)] exec_paths: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    #[allow(unused_variables)]
    fn target_kind<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
        target: Value<'v>,
    ) -> starlark::Result<&'static str> {
        Ok("")
    }

    #[allow(unused_variables)]
    fn collect_native_deps_dirs<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
        libraries: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(Depset::empty()))
    }

    #[allow(unused_variables)]
    fn get_runtime_classpath_for_archive<'v>(
        #[starlark(this)] _this: &JavaCommonInternal,
        jars: Value<'v>,
        excluded_jars: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(Depset::empty()))
    }
}

// ============================================================================
// JavaInfoProvider - Callable provider for Java compilation information
// ============================================================================

/// JavaInfo provider callable.
///
/// In Bazel, JavaInfo carries compilation outputs (jars, source jars),
/// compile-time classpath, and runtime classpath information.
/// Called as `JavaInfo(output_jar=..., compile_jar=..., ...)` to create instances.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct JavaInfoProvider;

impl JavaInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "JavaInfo".to_owned(),
            })
        })
    }
}

impl Display for JavaInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider JavaInfo>")
    }
}

starlark_simple_value!(JavaInfoProvider);

impl ProviderCallableLike for JavaInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "JavaInfo")]
impl<'v> StarlarkValue<'v> for JavaInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        create_native_provider_instance(JAVA_INFO_IDX, args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// JavaPluginInfoProvider - Callable provider for Java annotation processors
// ============================================================================

/// JavaPluginInfo provider callable.
///
/// Carries annotation processor information for Java compilation.
/// Called as `JavaPluginInfo(runtime_deps=..., processor_class=..., ...)` to create instances.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct JavaPluginInfoProvider;

impl JavaPluginInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "JavaPluginInfo".to_owned(),
            })
        })
    }
}

impl Display for JavaPluginInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider JavaPluginInfo>")
    }
}

starlark_simple_value!(JavaPluginInfoProvider);

impl ProviderCallableLike for JavaPluginInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "JavaPluginInfo")]
impl<'v> StarlarkValue<'v> for JavaPluginInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        create_native_provider_instance(JAVA_PLUGIN_INFO_IDX, args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// JavaRuntimeInfoProvider - Provider for Java runtime information
// ============================================================================

/// JavaRuntimeInfo provider callable (accessed via java_common.JavaRuntimeInfo).
///
/// Provides information about the Java runtime used during execution.
/// Called as `JavaRuntimeInfo(java_home=..., ...)` to create instances.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaRuntimeInfoProvider;

impl JavaRuntimeInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "JavaRuntimeInfo".to_owned(),
            })
        })
    }
}

impl Display for JavaRuntimeInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider JavaRuntimeInfo>")
    }
}

starlark_simple_value!(JavaRuntimeInfoProvider);

impl ProviderCallableLike for JavaRuntimeInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "JavaRuntimeInfo")]
impl<'v> StarlarkValue<'v> for JavaRuntimeInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        create_native_provider_instance(JAVA_RUNTIME_INFO_IDX, args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// JavaToolchainInfoProvider - Provider for Java toolchain information
// ============================================================================

/// JavaToolchainInfo provider callable (accessed via java_common.JavaToolchainInfo).
///
/// Provides information about the Java toolchain (javac, bootclasspath, etc.).
/// Called as `JavaToolchainInfo(javac=..., ...)` to create instances.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaToolchainInfoProvider;

impl JavaToolchainInfoProvider {
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "JavaToolchainInfo".to_owned(),
            })
        })
    }
}

impl Display for JavaToolchainInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider JavaToolchainInfo>")
    }
}

starlark_simple_value!(JavaToolchainInfoProvider);

impl ProviderCallableLike for JavaToolchainInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "JavaToolchainInfo")]
impl<'v> StarlarkValue<'v> for JavaToolchainInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        create_native_provider_instance(JAVA_TOOLCHAIN_INFO_IDX, args, eval)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register the java_common global and Java-related providers.
#[starlark_module]
pub fn register_java_common(globals: &mut GlobalsBuilder) {
    /// The java_common module for Java compilation utilities.
    const java_common: JavaCommonModule = JavaCommonModule;

    /// JavaInfo provider for Java compilation information.
    const JavaInfo: JavaInfoProvider = JavaInfoProvider;

    /// JavaPluginInfo provider for annotation processor information.
    const JavaPluginInfo: JavaPluginInfoProvider = JavaPluginInfoProvider;
}
