/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Global build configuration state for Bazel compatibility.
//!
//! Stores build-wide settings like `--compilation_mode` that are propagated
//! from the CLI through the daemon to the analysis phase.

use std::sync::RwLock;

/// Global build configuration that persists for the duration of a build command.
///
/// Values are set by the server when processing a client command, and read
/// by `ctx.var`, `ctx.fragments`, and `config_setting` evaluation.
static BUILD_CONFIG: RwLock<BuildConfig> = RwLock::new(BuildConfig {
    compilation_mode: None,
});

struct BuildConfig {
    /// Compilation mode: "fastbuild" (default), "dbg", or "opt".
    compilation_mode: Option<String>,
}

/// Set the compilation mode for the current build.
/// Called from the server when processing the client context.
pub fn set_compilation_mode(mode: &str) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.compilation_mode = if mode.is_empty() {
            None
        } else {
            Some(mode.to_owned())
        };
    }
}

/// Get the current compilation mode. Returns "fastbuild" if not set.
pub fn get_compilation_mode() -> String {
    BUILD_CONFIG
        .read()
        .ok()
        .and_then(|c| c.compilation_mode.clone())
        .unwrap_or_else(|| "fastbuild".to_owned())
}
