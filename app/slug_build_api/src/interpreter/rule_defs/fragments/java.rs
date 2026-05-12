/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Java configuration fragment.

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::AllocList;
use starlark::values::starlark_value;

// ============================================================================
// JavaFragment - Java configuration fragment
// ============================================================================

/// Java configuration fragment.
///
/// Accessed via `ctx.fragments.java`. Contains Java build settings used by
/// rules_java's Starlark implementations.
///
/// Reference: https://bazel.build/rules/lib/fragments/java
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaFragment;

impl Display for JavaFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<java fragment>")
    }
}

starlark_simple_value!(JavaFragment);

#[starlark_value(type = "java_fragment")]
impl<'v> StarlarkValue<'v> for JavaFragment {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(java_fragment_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "default_javac_flags"
                | "default_javac_flags_depset"
                | "default_jvm_opts"
                | "source_version"
                | "target_version"
                | "plugins"
                | "one_version_enforcement_level"
                | "multi_release_deploy_jars"
                | "bytecode_optimization_pass_actions"
                | "bytecode_optimizer_mnemonic"
                | "split_bytecode_optimization_pass"
                | "run_android_lint"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "default_javac_flags"
            | "default_jvm_opts"
            | "plugins"
            | "default_javac_flags_depset" => Some(heap.alloc(AllocList::EMPTY)),
            "source_version" | "target_version" => Some(heap.alloc("11")),
            "one_version_enforcement_level" => Some(heap.alloc("OFF")),
            "bytecode_optimizer_mnemonic" => Some(heap.alloc("Optimizer")),
            "bytecode_optimization_pass_actions" => Some(heap.alloc(0)),
            "multi_release_deploy_jars"
            | "split_bytecode_optimization_pass"
            | "run_android_lint" => Some(Value::new_bool(false)),
            _ => None,
        }
    }
}

#[starlark_module]
fn java_fragment_methods(builder: &mut MethodsBuilder) {
    /// Whether to use interface JARs for faster rebuilds.
    fn use_ijars(#[allow(unused_variables)] this: &JavaFragment) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Whether to use header compilation for direct deps.
    fn use_header_compilation_direct_deps(
        #[allow(unused_variables)] this: &JavaFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Strict Java deps enforcement level: "OFF", "WARN", "ERROR", "DEFAULT".
    fn strict_java_deps(
        #[allow(unused_variables)] this: &JavaFragment,
    ) -> starlark::Result<String> {
        Ok("DEFAULT".to_owned())
    }

    /// Whether java_import exports are restricted.
    fn disallow_java_import_exports(
        #[allow(unused_variables)] this: &JavaFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether explicit Java test dependencies should be enforced.
    fn enforce_explicit_java_test_deps(
        #[allow(unused_variables)] this: &JavaFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }
}
