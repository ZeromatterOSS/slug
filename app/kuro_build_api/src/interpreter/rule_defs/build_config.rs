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
//! Stores build-wide settings like `--compilation_mode` and `--define` that are
//! propagated from the CLI through the daemon to the analysis phase.

use std::collections::HashMap;
use std::sync::RwLock;

/// Global build configuration that persists for the duration of a build command.
///
/// Values are set by the server when processing a client command, and read
/// by `ctx.var`, `ctx.fragments`, and `config_setting` evaluation.
static BUILD_CONFIG: RwLock<BuildConfig> = RwLock::new(BuildConfig {
    compilation_mode: None,
    defines: None,
});

struct BuildConfig {
    /// Compilation mode: "fastbuild" (default), "dbg", or "opt".
    compilation_mode: Option<String>,
    /// --define KEY=VALUE pairs from the command line.
    defines: Option<HashMap<String, String>>,
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

/// Set --define values for the current build.
/// Each entry should be "KEY=VALUE" format.
pub fn set_defines(define_values: &[String]) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        let mut map = HashMap::new();
        for entry in define_values {
            if let Some((key, value)) = entry.split_once('=') {
                map.insert(key.to_owned(), value.to_owned());
            }
        }
        config.defines = if map.is_empty() { None } else { Some(map) };
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

/// Get the value of a --define variable, or None if not set.
pub fn get_define(key: &str) -> Option<String> {
    BUILD_CONFIG
        .read()
        .ok()
        .and_then(|c| c.defines.as_ref().and_then(|d| d.get(key).cloned()))
}

/// Get all --define values as a map.
pub fn get_all_defines() -> HashMap<String, String> {
    BUILD_CONFIG
        .read()
        .ok()
        .and_then(|c| c.defines.clone())
        .unwrap_or_default()
}
