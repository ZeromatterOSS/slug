/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! `RepositoryOs` — information about the host operating system exposed as
//! `module_ctx.os` / `repository_ctx.os`.

use allocative::Allocative;
use derive_more::Display;
use starlark::any::ProvidesStaticType;
use starlark::collections::SmallMap;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::typing::Ty;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::ValueLike;
use starlark::values::dict::Dict;
use starlark::values::starlark_value;

// ============================================================================
// RepositoryOs - Information about the host OS (simple value, no lifetime)
// ============================================================================

/// Information about the host operating system.
#[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative, Clone)]
#[display("<repository_os>")]
pub struct RepositoryOs {
    /// The OS name (e.g., "linux", "macos", "windows").
    pub(super) name: String,
    /// The CPU architecture (e.g., "x86_64", "aarch64").
    pub(super) arch: String,
}

starlark_simple_value!(RepositoryOs);

impl RepositoryOs {
    pub fn new() -> Self {
        let name = if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "mac os x"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "unknown"
        };

        let arch = if cfg!(target_arch = "x86_64") {
            "amd64"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else if cfg!(target_arch = "x86") {
            "x86_32"
        } else {
            "unknown"
        };

        Self {
            name: name.to_owned(),
            arch: arch.to_owned(),
        }
    }
}

#[starlark_value(type = "repository_os")]
impl<'v> StarlarkValue<'v> for RepositoryOs {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(repository_os_methods)
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any()
    }
}

#[starlark_module]
fn repository_os_methods(builder: &mut MethodsBuilder) {
    /// The OS name (e.g., "linux", "mac os x", "windows").
    #[starlark(attribute)]
    fn name(this: &RepositoryOs) -> starlark::Result<String> {
        Ok(this.name.clone())
    }

    /// The CPU architecture (e.g., "amd64", "aarch64").
    #[starlark(attribute)]
    fn arch(this: &RepositoryOs) -> starlark::Result<String> {
        Ok(this.arch.clone())
    }

    /// A snapshot of the environment variables at the time repository rules are executed.
    #[starlark(attribute)]
    fn environ<'v>(this: &RepositoryOs, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let mut map = SmallMap::new();
        for (key, val) in std::env::vars() {
            map.insert_hashed(
                heap.alloc_str(&key).to_value().get_hashed().unwrap(),
                heap.alloc_str(&val).to_value(),
            );
        }
        Ok(heap.alloc(Dict::new(map)))
    }
}
