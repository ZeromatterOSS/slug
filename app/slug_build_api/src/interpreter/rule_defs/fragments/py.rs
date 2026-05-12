/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Python configuration fragments.

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::starlark_value;

// ============================================================================
// PyFragment - Python configuration fragment
// ============================================================================

/// Python configuration fragment.
///
/// Accessed via `ctx.fragments.py`. Contains Python build settings used by
/// rules_python's Starlark implementations.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PyFragment;

impl Display for PyFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<py fragment>")
    }
}

starlark_simple_value!(PyFragment);

#[starlark_value(type = "py_fragment")]
impl<'v> StarlarkValue<'v> for PyFragment {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(py_fragment_methods)
    }
}

#[starlark_module]
fn py_fragment_methods(builder: &mut MethodsBuilder) {
    /// Whether native Python rules are disallowed.
    /// False means check_native_allowed() early-returns without error.
    #[starlark(attribute)]
    fn disallow_native_rules(
        #[allow(unused_variables)] this: &PyFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether Python 2 is disabled.
    #[starlark(attribute)]
    fn disable_py2(#[allow(unused_variables)] this: &PyFragment) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Whether __init__.py must be explicitly provided.
    /// False = auto-create __init__.py files.
    #[starlark(attribute)]
    fn default_to_explicit_init_py(
        #[allow(unused_variables)] this: &PyFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether to build a .zip archive of Python sources.
    /// Default is false on Linux/macOS, true on Windows.
    #[starlark(attribute)]
    fn build_python_zip(#[allow(unused_variables)] this: &PyFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether to use Python toolchains for interpreter resolution.
    #[starlark(attribute)]
    fn use_toolchains(#[allow(unused_variables)] this: &PyFragment) -> starlark::Result<bool> {
        Ok(false)
    }
}

// ============================================================================
// BazelPyFragment - Bazel-specific Python configuration fragment
// ============================================================================

/// Bazel-specific Python configuration fragment.
///
/// Accessed via `ctx.fragments.bazel_py`. Contains Bazel-specific Python settings.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct BazelPyFragment;

impl Display for BazelPyFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<bazel_py fragment>")
    }
}

starlark_simple_value!(BazelPyFragment);

#[starlark_value(type = "bazel_py_fragment")]
impl<'v> StarlarkValue<'v> for BazelPyFragment {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(bazel_py_fragment_methods)
    }
}

#[starlark_module]
fn bazel_py_fragment_methods(builder: &mut MethodsBuilder) {
    /// Whether to import all repositories into Python path.
    #[starlark(attribute)]
    fn python_import_all_repositories(
        #[allow(unused_variables)] this: &BazelPyFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// The Python interpreter path from --python_path flag.
    #[starlark(attribute)]
    fn python_path(#[allow(unused_variables)] this: &BazelPyFragment) -> starlark::Result<String> {
        use std::sync::OnceLock;
        static CACHED_PYTHON_PATH: OnceLock<String> = OnceLock::new();
        let path = CACHED_PYTHON_PATH.get_or_init(|| {
            let finder = if cfg!(windows) { "where" } else { "which" };
            for name in &["python3", "python"] {
                if let Ok(output) = std::process::Command::new(finder).arg(name).output() {
                    if output.status.success() {
                        // `where` on Windows may return multiple lines; take the first
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let p = stdout.lines().next().unwrap_or("").trim().to_owned();
                        if !p.is_empty() {
                            return p;
                        }
                    }
                }
            }
            if cfg!(windows) {
                "python.exe".to_owned()
            } else {
                "python3".to_owned()
            }
        });
        Ok(path.clone())
    }
}
