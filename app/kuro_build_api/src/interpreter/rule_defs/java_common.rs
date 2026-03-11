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
//! This provides a stub implementation of Bazel's java_common built-in module
//! that Java rules (rules_java) require for Java compilation support.
//!
//! ## Symbols
//!
//! - `java_common` - Module with Java compilation utilities
//! - `JavaInfo` - Provider for Java compilation information
//! - `JavaPluginInfo` - Provider for Java annotation processors
//!
//! Reference: https://bazel.build/rules/lib/toplevel/java_common

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
use starlark::values::list::AllocList;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

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
        )
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "INCOMPATIBLE_ENABLE_JAVA_TOOLCHAIN_RESOLUTION" => Some(Value::new_bool(false)),
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
        #[starlark(require = named, default = NoneOr::None)] annotation_processor_additional_outputs: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] strict_deps: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] java_toolchain: NoneOr<Value<'v>>,
        #[starlark(require = named, default = NoneOr::None)] neverlink: NoneOr<bool>,
        #[starlark(require = named, default = NoneOr::None)] enable_compile_jar_action: NoneOr<bool>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(java_common): Implement actual Java compilation
        Ok(NoneType)
    }

    /// Merges multiple JavaInfo providers into one.
    #[allow(unused_variables)]
    fn merge<'v>(
        #[starlark(this)] _this: &JavaCommonModule,
        #[starlark(require = pos)] providers: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(java_common): Implement JavaInfo merging
        Ok(NoneType)
    }

    /// Creates a JavaInfo provider from a compiled jar.
    #[allow(unused_variables)]
    fn make_non_strict<'v>(
        #[starlark(this)] _this: &JavaCommonModule,
        #[starlark(require = pos)] java_info: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Returns the default Java toolchain bootclasspath.
    fn boot_class_path<'v>(
        #[starlark(this)] _this: &JavaCommonModule,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }
}

// ============================================================================
// JavaInfoProvider - Provider for Java compilation information
// ============================================================================

/// JavaInfo provider stub.
///
/// In Bazel, JavaInfo carries compilation outputs (jars, source jars),
/// compile-time classpath, and runtime classpath information.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct JavaInfoProvider;

impl Display for JavaInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider JavaInfo>")
    }
}

starlark_simple_value!(JavaInfoProvider);

#[starlark_value(type = "JavaInfo")]
impl<'v> StarlarkValue<'v> for JavaInfoProvider {}

// ============================================================================
// JavaPluginInfoProvider - Provider for Java annotation processors
// ============================================================================

/// JavaPluginInfo provider stub.
///
/// Carries annotation processor information for Java compilation.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct JavaPluginInfoProvider;

impl Display for JavaPluginInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider JavaPluginInfo>")
    }
}

starlark_simple_value!(JavaPluginInfoProvider);

#[starlark_value(type = "JavaPluginInfo")]
impl<'v> StarlarkValue<'v> for JavaPluginInfoProvider {}

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
