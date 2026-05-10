/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! FeatureConfiguration and CcToolchainFeatures — Bazel cc feature wiring.

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::cc_common::host::is_windows_host;

// ============================================================================
// FeatureConfiguration - C++ feature configuration
// ============================================================================

/// FeatureConfiguration holds the enabled features for C++ compilation.
///
/// This is created by cc_common.configure_features() and used to control
/// which compiler flags and behaviors are enabled.
///
/// In Bazel, feature configuration is computed from the toolchain's declared
/// features combined with requested_features and unsupported_features.
/// We approximate this by maintaining a set of enabled feature names.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct FeatureConfiguration {
    /// Set of enabled feature names
    pub(crate) enabled_features: Vec<String>,
}

/// Default features that are typically enabled by CC toolchains.
/// These match common Bazel CC toolchain defaults.
///
/// `fastbuild` / `dbg` / `opt` are intentionally excluded: rules_cc activates
/// exactly one of them via `configure_features()`'s `requested_features`
/// (sourced from `ctx.fragments.cpp.compilation_mode()`). Having all three
/// always enabled would prevent `FeatureConfiguration::is_feature_enabled`
/// from distinguishing modes in `get_memory_inefficient_command_line`.
pub(crate) fn default_cc_features() -> Vec<&'static str> {
    let mut features = vec![
        // Core features always enabled
        "supports_dynamic_linker",
        "supports_start_end_lib",
        "compiler_param_file",
        "linker_param_file",
        // Standard features
        "no_legacy_features",
        "dependency_file",
        "random_seed",
        "per_object_debug_info",
        "preprocessor_defines",
        "includes",
        "include_paths",
        "user_compile_flags",
        "sysroot",
        // Link features
        "shared_flag",
        "linkstamps",
        "output_execpath_flags",
        "runtime_library_search_directories",
        "library_search_directories",
        "archiver_flags",
        "libraries_to_link",
        "force_pic_flags",
        "user_link_flags",
        "strip_debug_symbols",
    ];
    // Platform-specific defaults
    if !is_windows_host() {
        features.push("pic");
        features.push("supports_pic");
    } else {
        features.push("targets_windows");
        features.push("copy_dynamic_libraries_to_binary");
        features.push("has_configured_linker_path");
        features.push("no_stripping");
        // Windows-only: interface libraries (.lib) are native to MSVC. On
        // Linux/macOS, enabling this flag makes rules_cc's finalize_link_action
        // route through link_dynamic_library.sh when `has_configured_linker_path`
        // isn't enabled, which our default toolchain doesn't want — we drive
        // the linker directly via `get_tool_for_action`.
        features.push("supports_interface_shared_libraries");
    }
    features
}

impl Default for FeatureConfiguration {
    fn default() -> Self {
        Self {
            enabled_features: default_cc_features()
                .into_iter()
                .map(|s| s.to_owned())
                .collect(),
        }
    }
}

impl FeatureConfiguration {
    /// Create a feature configuration from requested and unsupported features.
    pub fn new(requested_features: Vec<String>, unsupported_features: Vec<String>) -> Self {
        let mut enabled: Vec<String> = default_cc_features()
            .into_iter()
            .map(|s| s.to_owned())
            .collect();

        // Add requested features
        for f in &requested_features {
            if !enabled.iter().any(|e| e == f) {
                enabled.push(f.clone());
            }
        }

        // Remove unsupported features
        enabled.retain(|f| !unsupported_features.contains(f));

        Self {
            enabled_features: enabled,
        }
    }

    /// Check if a feature is enabled.
    pub fn is_feature_enabled(&self, feature_name: &str) -> bool {
        self.enabled_features.iter().any(|f| f == feature_name)
    }
}

impl Display for FeatureConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "FeatureConfiguration(features=[{}])",
            self.enabled_features.len()
        )
    }
}

starlark_simple_value!(FeatureConfiguration);

#[starlark_value(type = "FeatureConfiguration")]
impl<'v> StarlarkValue<'v> for FeatureConfiguration {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(feature_configuration_methods)
    }
}

#[starlark_module]
fn feature_configuration_methods(builder: &mut MethodsBuilder) {
    /// Returns whether a given feature is enabled.
    fn is_enabled(
        #[starlark(this)] this: &FeatureConfiguration,
        feature: &str,
    ) -> starlark::Result<bool> {
        Ok(this.is_feature_enabled(feature))
    }

    /// Returns whether a given feature was requested (i.e., is enabled).
    /// In Bazel, is_requested checks if the feature was explicitly requested,
    /// but for our purposes it's equivalent to is_enabled.
    fn is_requested(
        #[starlark(this)] this: &FeatureConfiguration,
        feature: &str,
    ) -> starlark::Result<bool> {
        Ok(this.is_feature_enabled(feature))
    }
}

// ============================================================================
// CcToolchainFeatures - Toolchain feature configuration object
// ============================================================================

/// Object returned by `cc_common_internal.cc_toolchain_features()`.
///
/// Stores the feature names and action config names from the CcToolchainConfigInfo.
/// Provides `configure_features()` which creates a FeatureConfiguration and
/// `default_features_and_action_configs()` which returns the list of default
/// feature/action_config names.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Clone)]
pub struct CcToolchainFeatures {
    /// Names of features declared in the toolchain config
    pub(crate) feature_names: Vec<String>,
    /// Names of features that are enabled by default
    pub(crate) default_enabled_features: Vec<String>,
    /// Names of action configs declared in the toolchain config
    pub(crate) action_config_names: Vec<String>,
    /// The tools directory path (reserved for future use in tool path resolution)
    #[allow(dead_code)]
    pub(crate) tools_directory: String,
}

impl CcToolchainFeatures {
    pub fn empty() -> Self {
        Self {
            feature_names: Vec::new(),
            default_enabled_features: Vec::new(),
            action_config_names: Vec::new(),
            tools_directory: String::new(),
        }
    }
}

impl Display for CcToolchainFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CcToolchainFeatures(features={}, action_configs={})",
            self.feature_names.len(),
            self.action_config_names.len()
        )
    }
}

starlark_simple_value!(CcToolchainFeatures);

#[starlark_value(type = "CcToolchainFeatures")]
impl<'v> StarlarkValue<'v> for CcToolchainFeatures {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_toolchain_features_methods)
    }
}

#[starlark_module]
fn cc_toolchain_features_methods(builder: &mut MethodsBuilder) {
    /// Creates a FeatureConfiguration from the requested features.
    ///
    /// Called by rules_cc's configure_features.bzl to produce the final
    /// feature configuration for a cc_toolchain.
    fn configure_features<'v>(
        #[starlark(this)] this: &CcToolchainFeatures,
        #[starlark(require = named)] requested_features: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<FeatureConfiguration> {
        let heap = eval.heap();
        let mut requested: Vec<String> = Vec::new();
        if let Ok(iter) = requested_features.iterate(heap) {
            for item in iter {
                if let Some(s) = item.unpack_str() {
                    requested.push(s.to_owned());
                }
            }
        }

        // Start with the default platform features
        let mut enabled: Vec<String> = default_cc_features()
            .into_iter()
            .map(|s| s.to_owned())
            .collect();

        // Add features that are enabled by default in the toolchain config
        for f in &this.default_enabled_features {
            if !enabled.iter().any(|e| e == f) {
                enabled.push(f.clone());
            }
        }

        // Add all requested features
        for f in &requested {
            if !enabled.iter().any(|e| e == f) {
                enabled.push(f.clone());
            }
        }

        Ok(FeatureConfiguration {
            enabled_features: enabled,
        })
    }

    /// Returns the list of default feature and action_config names.
    ///
    /// Called by rules_cc's configure_features.bzl (line 146) to collect
    /// default features from the toolchain config.
    fn default_features_and_action_configs(
        #[starlark(this)] this: &CcToolchainFeatures,
    ) -> starlark::Result<Vec<String>> {
        let mut result: Vec<String> = Vec::new();
        // Add features that are enabled by default
        for f in &this.default_enabled_features {
            result.push(f.clone());
        }
        // Add all action config names (they act as implicit features)
        for a in &this.action_config_names {
            if !result.contains(a) {
                result.push(a.clone());
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compilation_mode_is_not_default_enabled() {
        // After Plan 19.6, "opt" / "dbg" / "fastbuild" must NOT appear in the
        // default feature set — they are activated per-cfg via
        // configure_features's `requested_features`.
        let defaults = default_cc_features();
        assert!(!defaults.contains(&"opt"));
        assert!(!defaults.contains(&"dbg"));
        assert!(!defaults.contains(&"fastbuild"));
    }

    #[test]
    fn requesting_opt_enables_opt_only() {
        let fc = FeatureConfiguration::new(vec!["opt".to_owned()], vec![]);
        assert!(fc.is_feature_enabled("opt"));
        assert!(!fc.is_feature_enabled("dbg"));
        assert!(!fc.is_feature_enabled("fastbuild"));
    }

    #[test]
    fn requesting_dbg_enables_dbg_only() {
        let fc = FeatureConfiguration::new(vec!["dbg".to_owned()], vec![]);
        assert!(fc.is_feature_enabled("dbg"));
        assert!(!fc.is_feature_enabled("opt"));
    }
}
