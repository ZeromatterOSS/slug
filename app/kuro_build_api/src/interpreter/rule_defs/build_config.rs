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
    action_env: None,
    test_env: None,
    copts: None,
    cxxopts: None,
    conlyopts: None,
    linkopts: None,
    strip: None,
    features: None,
    stamp: false,
    collect_code_coverage: false,
});

struct BuildConfig {
    /// Compilation mode: "fastbuild" (default), "dbg", or "opt".
    compilation_mode: Option<String>,
    /// --define KEY=VALUE pairs from the command line.
    defines: Option<HashMap<String, String>>,
    /// --action_env NAME=VALUE pairs from the command line.
    action_env: Option<HashMap<String, String>>,
    /// --test_env NAME=VALUE pairs from the command line.
    test_env: Option<HashMap<String, String>>,
    /// --copt flags (C/C++ compilation flags).
    copts: Option<Vec<String>>,
    /// --cxxopt flags (C++-specific compilation flags).
    cxxopts: Option<Vec<String>>,
    /// --conlyopt flags (C-specific compilation flags).
    conlyopts: Option<Vec<String>>,
    /// --linkopt flags (linker flags).
    linkopts: Option<Vec<String>>,
    /// --strip flag: "always", "sometimes", or "never".
    strip: Option<String>,
    /// --features flags (enabled/disabled features like "static_link_cpp_runtimes").
    features: Option<Vec<String>>,
    /// --stamp / --nostamp flag.
    stamp: bool,
    /// --collect_code_coverage / --nocollect_code_coverage flag.
    collect_code_coverage: bool,
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

/// Set --action_env values for the current build.
/// Each entry should be "NAME=VALUE" or "NAME" (inherit from environment) format.
pub fn set_action_env(env_values: &[String]) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        let mut map = HashMap::new();
        for entry in env_values {
            if let Some((key, value)) = entry.split_once('=') {
                map.insert(key.to_owned(), value.to_owned());
            } else {
                // NAME without =VALUE means inherit from the process environment
                if let Ok(value) = std::env::var(entry) {
                    map.insert(entry.to_owned(), value);
                }
            }
        }
        config.action_env = if map.is_empty() { None } else { Some(map) };
    }
}

/// Get all --action_env values as a map.
pub fn get_action_env() -> HashMap<String, String> {
    BUILD_CONFIG
        .read()
        .ok()
        .and_then(|c| c.action_env.clone())
        .unwrap_or_default()
}

/// Set --copt values for the current build.
pub fn set_copts(values: &[String]) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.copts = if values.is_empty() { None } else { Some(values.to_vec()) };
    }
}

/// Get --copt values.
pub fn get_copts() -> Vec<String> {
    BUILD_CONFIG.read().ok().and_then(|c| c.copts.clone()).unwrap_or_default()
}

/// Set --cxxopt values for the current build.
pub fn set_cxxopts(values: &[String]) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.cxxopts = if values.is_empty() { None } else { Some(values.to_vec()) };
    }
}

/// Get --cxxopt values.
pub fn get_cxxopts() -> Vec<String> {
    BUILD_CONFIG.read().ok().and_then(|c| c.cxxopts.clone()).unwrap_or_default()
}

/// Set --conlyopt values for the current build.
pub fn set_conlyopts(values: &[String]) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.conlyopts = if values.is_empty() { None } else { Some(values.to_vec()) };
    }
}

/// Get --conlyopt values.
pub fn get_conlyopts() -> Vec<String> {
    BUILD_CONFIG.read().ok().and_then(|c| c.conlyopts.clone()).unwrap_or_default()
}

/// Set --linkopt values for the current build.
pub fn set_linkopts(values: &[String]) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.linkopts = if values.is_empty() { None } else { Some(values.to_vec()) };
    }
}

/// Get --linkopt values.
pub fn get_linkopts() -> Vec<String> {
    BUILD_CONFIG.read().ok().and_then(|c| c.linkopts.clone()).unwrap_or_default()
}

/// Set --strip value for the current build.
pub fn set_strip(value: &str) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.strip = if value.is_empty() { None } else { Some(value.to_owned()) };
    }
}

/// Get --strip value. Returns "sometimes" if not set.
pub fn get_strip() -> String {
    BUILD_CONFIG.read().ok().and_then(|c| c.strip.clone()).unwrap_or_else(|| "sometimes".to_owned())
}

/// Set --features values for the current build.
pub fn set_features(values: &[String]) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.features = if values.is_empty() { None } else { Some(values.to_vec()) };
    }
}

/// Get --features values.
pub fn get_features() -> Vec<String> {
    BUILD_CONFIG.read().ok().and_then(|c| c.features.clone()).unwrap_or_default()
}

/// Set --test_env values for the current build.
/// Each entry should be "NAME=VALUE" or "NAME" (inherit from environment) format.
pub fn set_test_env(env_values: &[String]) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        let mut map = HashMap::new();
        for entry in env_values {
            if let Some((key, value)) = entry.split_once('=') {
                map.insert(key.to_owned(), value.to_owned());
            } else {
                // NAME without =VALUE means inherit from the process environment
                if let Ok(value) = std::env::var(entry) {
                    map.insert(entry.to_owned(), value);
                }
            }
        }
        config.test_env = if map.is_empty() { None } else { Some(map) };
    }
}

/// Get all --test_env values as a map.
pub fn get_test_env() -> HashMap<String, String> {
    BUILD_CONFIG
        .read()
        .ok()
        .and_then(|c| c.test_env.clone())
        .unwrap_or_default()
}

/// Set --stamp flag for the current build.
pub fn set_stamp(enabled: bool) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.stamp = enabled;
    }
}

/// Get --stamp flag. Returns false if not set.
pub fn get_stamp() -> bool {
    BUILD_CONFIG.read().ok().map(|c| c.stamp).unwrap_or(false)
}

/// Set --collect_code_coverage flag for the current build.
pub fn set_collect_code_coverage(enabled: bool) {
    if let Ok(mut config) = BUILD_CONFIG.write() {
        config.collect_code_coverage = enabled;
    }
}

/// Get --collect_code_coverage flag. Returns false if not set.
pub fn get_collect_code_coverage() -> bool {
    BUILD_CONFIG.read().ok().map(|c| c.collect_code_coverage).unwrap_or(false)
}
