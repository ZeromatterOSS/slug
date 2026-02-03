/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Synthetic repository generation for known module extensions.
//!
//! Some BCR modules use module extensions that generate simple repositories.
//! Until full extension execution is implemented (Phase 5), this module provides
//! static implementations for well-known extensions.
//!
//! # Supported Extensions
//!
//! - `bazel_features` version_extension: Generates `@bazel_features_version` and
//!   `@bazel_features_globals` repositories containing version information.
//!
//! # Future Work
//!
//! Once module extension execution is fully implemented (Phase 5), these synthetic
//! repos can be removed in favor of proper extension execution.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;

use crate::types::ExtensionUsage;
use crate::types::ParsedModuleFile;

/// The Bazel version Kuro reports for compatibility.
/// Using "9.0.0" without a suffix so it's treated as a released version,
/// which compares greater than any prerelease (e.g., "9.0.0-pre.20250911").
pub const KURO_BAZEL_VERSION: &str = "9.0.0";

/// A synthetic repository generated for a known extension.
#[derive(Debug, Clone)]
pub struct SyntheticRepo {
    /// Repository name (e.g., "bazel_features_version").
    pub name: String,

    /// Files to create in the repository.
    /// Key is the relative path, value is the content.
    pub files: HashMap<String, String>,
}

/// Collect all extension-generated repos needed by parsed modules.
///
/// This function examines all extension usages across the dependency graph
/// and generates synthetic repos for known extensions.
///
/// Returns a list of synthetic repos that should be created and registered as cells.
pub fn collect_synthetic_repos(
    parsed_modules: &[(String, ParsedModuleFile)],
) -> Vec<SyntheticRepo> {
    let mut repos = Vec::new();
    let mut seen_extensions = std::collections::HashSet::new();

    for (_module_name, parsed) in parsed_modules {
        for usage in &parsed.extension_usages {
            // Create a unique key for this extension
            let ext_key = usage.extension_id();
            if seen_extensions.contains(&ext_key) {
                continue;
            }
            seen_extensions.insert(ext_key.clone());

            // Check for known extensions and generate synthetic repos
            if let Some(synthetic) = generate_synthetic_repos_for_extension(usage) {
                repos.extend(synthetic);
            }
        }
    }

    repos
}

/// Generate synthetic repos for a known extension.
///
/// Returns `Some` with the repos if this is a known extension,
/// `None` if the extension is not recognized.
fn generate_synthetic_repos_for_extension(usage: &ExtensionUsage) -> Option<Vec<SyntheticRepo>> {
    // Match known extensions by their bzl file and name
    match (usage.extension_bzl_file.as_str(), usage.extension_name.as_str()) {
        // bazel_features version_extension
        ("//private:extensions.bzl", "version_extension")
        | ("@bazel_features//private:extensions.bzl", "version_extension") => {
            Some(generate_bazel_features_repos())
        }
        // rules_cc cc_configure_extension
        ("//cc:extensions.bzl", "cc_configure_extension")
        | ("@rules_cc//cc:extensions.bzl", "cc_configure_extension") => {
            Some(generate_rules_cc_repos())
        }
        // rules_cc compatibility_proxy extension
        ("//cc:extensions.bzl", "compatibility_proxy")
        | ("@rules_cc//cc:extensions.bzl", "compatibility_proxy") => {
            Some(generate_cc_compatibility_repo())
        }
        _ => None,
    }
}

/// Generate the @bazel_features_version and @bazel_features_globals repos.
fn generate_bazel_features_repos() -> Vec<SyntheticRepo> {
    vec![
        generate_bazel_features_version_repo(),
        generate_bazel_features_globals_repo(),
    ]
}

/// Generate the @bazel_features_version repository.
///
/// This repo contains a single file `version.bzl` with the Bazel version string.
fn generate_bazel_features_version_repo() -> SyntheticRepo {
    let mut files = HashMap::new();

    // version.bzl - contains the version string
    files.insert(
        "version.bzl".to_string(),
        format!("version = \"{}\"\n", KURO_BAZEL_VERSION),
    );

    // BUILD.bazel - exports the .bzl file
    files.insert(
        "BUILD.bazel".to_string(),
        r#"exports_files(["version.bzl"])

# Note: bzl_library is from bazel_skylib, but we skip it for simplicity
"#
        .to_string(),
    );

    SyntheticRepo {
        name: "bazel_features_version".to_string(),
        files,
    }
}

/// Generate the @bazel_features_globals repository.
///
/// This repo contains a `globals.bzl` file with a struct of available globals
/// based on the Bazel version.
fn generate_bazel_features_globals_repo() -> SyntheticRepo {
    let mut files = HashMap::new();

    // globals.bzl - contains the globals struct based on version
    // For Bazel 9.0.0-kuro, we enable globals available in Bazel 9.0
    let globals_content = r#"# Auto-generated globals for Kuro (Bazel 9.0 compatible)
# This file is generated by kuro_bzlmod::synthetic_repos

globals = struct(
    # CcSharedLibraryHintInfo: 7.0.0-pre.20230316.2 to "" (no max)
    CcSharedLibraryHintInfo = getattr(getattr(native, 'legacy_globals', None), 'CcSharedLibraryHintInfo', CcSharedLibraryHintInfo) if hasattr(native, 'CcSharedLibraryHintInfo') else None,
    # PackageSpecificationInfo: 6.4.0 to "" (no max)
    PackageSpecificationInfo = getattr(getattr(native, 'legacy_globals', None), 'PackageSpecificationInfo', PackageSpecificationInfo) if hasattr(native, 'PackageSpecificationInfo') else None,
    # RunEnvironmentInfo: 5.3.0 to "" (no max)
    RunEnvironmentInfo = getattr(getattr(native, 'legacy_globals', None), 'RunEnvironmentInfo', RunEnvironmentInfo) if hasattr(native, 'RunEnvironmentInfo') else None,
    # set: 8.1.0 to "" (no max)
    set = getattr(getattr(native, 'legacy_globals', None), 'set', set) if hasattr(native, 'set') else None,
    # subrule: 7.0.0 to "" (no max)
    subrule = getattr(getattr(native, 'legacy_globals', None), 'subrule', subrule) if hasattr(native, 'subrule') else None,
    # DefaultInfo: 0.0.1 to "" (no max) - always available
    DefaultInfo = getattr(getattr(native, 'legacy_globals', None), 'DefaultInfo', DefaultInfo),
    # macro: 8.0.0 to "" (no max)
    macro = getattr(getattr(native, 'legacy_globals', None), 'macro', macro) if hasattr(native, 'macro') else None,
    # CcSharedLibraryInfo: 6.0.0-pre.20220630.1 to 9.0.0-pre.20250921.2 - NOT available in 9.0.0
    CcSharedLibraryInfo = None,
    # cc_proto_aspect: 7.0.0-pre.20230405.2 to 8.0.0 - NOT available in 9.0.0
    cc_proto_aspect = None,
    # JavaInfo: "" to 8.0.0 - NOT available in 9.0.0
    JavaInfo = None,
    # JavaPluginInfo: "" to 8.0.0 - NOT available in 9.0.0
    JavaPluginInfo = None,
    # ProtoInfo: "" to 8.0.0 - NOT available in 9.0.0
    ProtoInfo = None,
    # PyCcLinkParamsProvider: "" to 8.0.0 - NOT available in 9.0.0
    PyCcLinkParamsProvider = None,
    # PyInfo: "" to 8.0.0 - NOT available in 9.0.0
    PyInfo = None,
    # PyRuntimeInfo: "" to 8.0.0 - NOT available in 9.0.0
    PyRuntimeInfo = None,
    # __TestingOnly_NeverAvailable: 1000000000.0.0 to "" - never available
    __TestingOnly_NeverAvailable = None,
)
"#;

    files.insert("globals.bzl".to_string(), globals_content.to_string());

    // BUILD.bazel - exports the .bzl file
    files.insert(
        "BUILD.bazel".to_string(),
        r#"exports_files(["globals.bzl"])
"#
        .to_string(),
    );

    SyntheticRepo {
        name: "bazel_features_globals".to_string(),
        files,
    }
}

/// Generate the @local_config_cc and @local_config_cc_toolchains repos.
///
/// These are created by rules_cc's cc_configure_extension.
fn generate_rules_cc_repos() -> Vec<SyntheticRepo> {
    vec![
        generate_local_config_cc_repo(),
        generate_local_config_cc_toolchains_repo(),
    ]
}

/// Generate the @local_config_cc repository.
///
/// This contains the detected C++ toolchain configuration.
fn generate_local_config_cc_repo() -> SyntheticRepo {
    let mut files = HashMap::new();

    // BUILD.bazel with basic toolchain detection
    // This is a simplified version - full implementation would detect system compiler
    // Note: We don't use package(default_visibility=...) because that's Bazel-specific
    // and doesn't work in Kuro's Buck2-based model. Instead, we set visibility explicitly.
    // Also, we use native cc_toolchain_suite and cc_toolchain instead of loading from
    // rules_cc, because the rules_cc versions have additional required implicit attributes.
    let build_content = r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# This is a simplified toolchain configuration

load(":local_config.bzl", "local_config")

# Placeholder toolchain - actual detection happens at build time
cc_toolchain_suite(
    name = "toolchain",
    toolchains = {
        "k8": ":cc-compiler-k8",
        "k8|gcc": ":cc-compiler-k8",
        "k8|clang": ":cc-compiler-k8",
    },
    visibility = ["//visibility:public"],
)

filegroup(
    name = "empty",
    srcs = [],
    visibility = ["//visibility:public"],
)

cc_toolchain(
    name = "cc-compiler-k8",
    all_files = ":empty",
    ar_files = ":empty",
    as_files = ":empty",
    compiler_files = ":empty",
    dwp_files = ":empty",
    linker_files = ":empty",
    objcopy_files = ":empty",
    strip_files = ":empty",
    supports_param_files = True,
    toolchain_config = ":local_toolchain_config",
    toolchain_identifier = "local",
    visibility = ["//visibility:public"],
)

# Instantiate the toolchain config rule
local_config(
    name = "local_toolchain_config",
    visibility = ["//visibility:public"],
)
"#;

    files.insert("BUILD.bazel".to_string(), build_content.to_string());

    // local_config.bzl - the rule definition must be in a .bzl file
    let bzl_content = r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# Minimal toolchain configuration rule

load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "tool_path")

def _impl(ctx):
    return cc_common.create_cc_toolchain_config_info(
        ctx = ctx,
        toolchain_identifier = "local",
        host_system_name = "local",
        target_system_name = "local",
        target_cpu = "k8",
        target_libc = "local",
        compiler = "gcc",
        abi_version = "local",
        abi_libc_version = "local",
        tool_paths = [
            tool_path(name = "gcc", path = "/usr/bin/gcc"),
            tool_path(name = "ld", path = "/usr/bin/ld"),
            tool_path(name = "ar", path = "/usr/bin/ar"),
            tool_path(name = "cpp", path = "/usr/bin/cpp"),
            tool_path(name = "gcov", path = "/usr/bin/gcov"),
            tool_path(name = "nm", path = "/usr/bin/nm"),
            tool_path(name = "objdump", path = "/usr/bin/objdump"),
            tool_path(name = "strip", path = "/usr/bin/strip"),
        ],
    )

local_config = rule(
    implementation = _impl,
    attrs = {},
    provides = [CcToolchainConfigInfo],
)
"#;

    files.insert("local_config.bzl".to_string(), bzl_content.to_string());

    SyntheticRepo {
        name: "local_config_cc".to_string(),
        files,
    }
}

/// Generate the @local_config_cc_toolchains repository.
fn generate_local_config_cc_toolchains_repo() -> SyntheticRepo {
    let mut files = HashMap::new();

    // BUILD.bazel with toolchain registrations
    // Note: We don't use package(default_visibility=...) because that's Bazel-specific
    // and doesn't work in Kuro's Buck2-based model. Instead, we set visibility explicitly.
    let build_content = r#"# Auto-generated by kuro_bzlmod::synthetic_repos

toolchain(
    name = "cc-toolchain-k8",
    exec_compatible_with = [
        "@platforms//cpu:x86_64",
        "@platforms//os:linux",
    ],
    target_compatible_with = [
        "@platforms//cpu:x86_64",
        "@platforms//os:linux",
    ],
    toolchain = "@local_config_cc//:cc-compiler-k8",
    toolchain_type = "@rules_cc//cc:toolchain_type",
    visibility = ["//visibility:public"],
)

# Alias for :all pattern
alias(
    name = "all",
    actual = ":cc-toolchain-k8",
    visibility = ["//visibility:public"],
)
"#;

    files.insert("BUILD.bazel".to_string(), build_content.to_string());

    SyntheticRepo {
        name: "local_config_cc_toolchains".to_string(),
        files,
    }
}

/// Generate the @cc_compatibility_proxy repository.
///
/// For Bazel 9.0+, this repo provides proxy files that load from rules_cc's
/// private implementation modules.
fn generate_cc_compatibility_repo() -> Vec<SyntheticRepo> {
    let mut files = HashMap::new();

    // BUILD.bazel
    let build_content = r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# Compatibility proxy for rules_cc (Bazel 9.0+ mode)

load("@bazel_skylib//:bzl_library.bzl", "bzl_library")

bzl_library(
  name = "proxy_bzl",
  srcs = ["proxy.bzl"],
  deps = [
    "@rules_cc//cc/private/rules_impl:core_rules",
    "@rules_cc//cc/private/rules_impl:toolchain_rules",
    "@rules_cc//cc/private:cc_common",
  ],
  visibility = ["@rules_cc//cc:__subpackages__"],
)

bzl_library(
  name = "symbols_bzl",
  srcs = ["symbols.bzl"],
  deps = [
    "@rules_cc//cc/private:cc_common",
    "@rules_cc//cc/private:cc_shared_library_info_bzl",
    "@rules_cc//cc/private/toolchain_config:toolchain_config_bzl",
  ],
  visibility = ["@rules_cc//cc:__subpackages__"],
)
"#;
    files.insert("BUILD".to_string(), build_content.to_string());

    // proxy.bzl - rule implementations
    let proxy_content = r#"# Auto-generated by kuro_bzlmod::synthetic_repos

load("@rules_cc//cc/private/rules_impl:cc_binary.bzl", _cc_binary = "cc_binary")
load("@rules_cc//cc/private/rules_impl:cc_import.bzl", _cc_import = "cc_import")
load("@rules_cc//cc/private/rules_impl:cc_library.bzl", _cc_library = "cc_library")
load("@rules_cc//cc/private/rules_impl:cc_shared_library.bzl", _cc_shared_library = "cc_shared_library")
load("@rules_cc//cc/private/rules_impl:cc_static_library.bzl", _cc_static_library = "cc_static_library")
load("@rules_cc//cc/private/rules_impl:cc_test.bzl", _cc_test = "cc_test")
load("@rules_cc//cc/private/rules_impl:objc_import.bzl", _objc_import = "objc_import")
load("@rules_cc//cc/private/rules_impl:objc_library.bzl", _objc_library = "objc_library")
load("@rules_cc//cc/private/rules_impl/fdo:fdo_prefetch_hints.bzl", _fdo_prefetch_hints = "fdo_prefetch_hints")
load("@rules_cc//cc/private/rules_impl/fdo:fdo_profile.bzl", _fdo_profile = "fdo_profile")
load("@rules_cc//cc/private/rules_impl/fdo:memprof_profile.bzl", _memprof_profile = "memprof_profile")
load("@rules_cc//cc/private/rules_impl/fdo:propeller_optimize.bzl", _propeller_optimize = "propeller_optimize")
load("@rules_cc//cc/private/rules_impl:cc_toolchain.bzl", _cc_toolchain = "cc_toolchain")
load("@rules_cc//cc/private/rules_impl:cc_toolchain_alias.bzl", _cc_toolchain_alias = "cc_toolchain_alias")

cc_binary = _cc_binary
cc_import = _cc_import
cc_library = _cc_library
cc_shared_library = _cc_shared_library
cc_static_library = _cc_static_library
cc_test = _cc_test
objc_import = _objc_import
objc_library = _objc_library
fdo_prefetch_hints = _fdo_prefetch_hints
fdo_profile = _fdo_profile
memprof_profile = _memprof_profile
propeller_optimize = _propeller_optimize
cc_toolchain = _cc_toolchain
cc_toolchain_alias = _cc_toolchain_alias
"#;
    files.insert("proxy.bzl".to_string(), proxy_content.to_string());

    // symbols.bzl - provider and cc_common exports
    let symbols_content = r#"# Auto-generated by kuro_bzlmod::synthetic_repos

load("@rules_cc//cc/private:cc_common.bzl", _cc_common = "cc_common")
load("@rules_cc//cc/private:cc_info.bzl", _CcInfo = "CcInfo")
load("@rules_cc//cc/private:cc_shared_library_info.bzl", _CcSharedLibraryInfo = "CcSharedLibraryInfo")
load("@rules_cc//cc/private:debug_package_info.bzl", _DebugPackageInfo = "DebugPackageInfo")
load("@rules_cc//cc/private:objc_info.bzl", _ObjcInfo = "ObjcInfo")
load("@rules_cc//cc/private/toolchain_config:cc_toolchain_config_info.bzl", _CcToolchainConfigInfo = "CcToolchainConfigInfo")

cc_common = _cc_common
CcInfo = _CcInfo
DebugPackageInfo = _DebugPackageInfo
CcToolchainConfigInfo = _CcToolchainConfigInfo
ObjcInfo = _ObjcInfo
new_objc_provider = _ObjcInfo
CcSharedLibraryInfo = _CcSharedLibraryInfo
"#;
    files.insert("symbols.bzl".to_string(), symbols_content.to_string());

    vec![SyntheticRepo {
        name: "cc_compatibility_proxy".to_string(),
        files,
    }]
}

/// Materialize synthetic repos to the filesystem.
///
/// Creates the repo directories and writes all files.
pub fn materialize_synthetic_repos(
    repos: &[SyntheticRepo],
    base_dir: &Path,
) -> anyhow::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for repo in repos {
        let repo_path = base_dir.join(&repo.name);
        fs::create_dir_all(&repo_path)
            .with_context(|| format!("Failed to create synthetic repo directory: {:?}", repo_path))?;

        for (file_path, content) in &repo.files {
            let full_path = repo_path.join(file_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut file = fs::File::create(&full_path)
                .with_context(|| format!("Failed to create file: {:?}", full_path))?;
            file.write_all(content.as_bytes())?;
        }

        paths.push(repo_path);
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_bazel_features_version_repo() {
        let repo = generate_bazel_features_version_repo();
        assert_eq!(repo.name, "bazel_features_version");
        assert!(repo.files.contains_key("version.bzl"));
        assert!(repo.files.contains_key("BUILD.bazel"));

        let version_content = repo.files.get("version.bzl").unwrap();
        assert!(version_content.contains("9.0.0"));
    }

    #[test]
    fn test_generate_bazel_features_globals_repo() {
        let repo = generate_bazel_features_globals_repo();
        assert_eq!(repo.name, "bazel_features_globals");
        assert!(repo.files.contains_key("globals.bzl"));
        assert!(repo.files.contains_key("BUILD.bazel"));

        let globals_content = repo.files.get("globals.bzl").unwrap();
        assert!(globals_content.contains("globals = struct("));
        assert!(globals_content.contains("DefaultInfo"));
    }

    #[test]
    fn test_collect_synthetic_repos() {
        let mut parsed = ParsedModuleFile {
            module: crate::types::Module::new("test".to_string(), crate::version::Version::parse("1.0.0").unwrap()),
            has_module_directive: true,
            extension_usages: vec![],
        };

        // Add bazel_features extension usage
        let mut usage = crate::types::ExtensionUsage::new(
            "//private:extensions.bzl".to_string(),
            "version_extension".to_string(),
        );
        let mut use_repo = crate::types::UseRepo::new();
        use_repo.repos.push("bazel_features_version".to_string());
        use_repo.repos.push("bazel_features_globals".to_string());
        usage.imports.push(use_repo);
        parsed.extension_usages.push(usage);

        let repos = collect_synthetic_repos(&[("bazel_features".to_string(), parsed)]);
        assert_eq!(repos.len(), 2);

        let names: Vec<_> = repos.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"bazel_features_version"));
        assert!(names.contains(&"bazel_features_globals"));
    }

    #[test]
    fn test_materialize_synthetic_repos() {
        let temp_dir = tempfile::tempdir().unwrap();
        let repos = generate_bazel_features_repos();

        let paths = materialize_synthetic_repos(&repos, temp_dir.path()).unwrap();
        assert_eq!(paths.len(), 2);

        // Check version repo
        let version_path = temp_dir.path().join("bazel_features_version");
        assert!(version_path.exists());
        assert!(version_path.join("version.bzl").exists());
        assert!(version_path.join("BUILD.bazel").exists());

        // Check globals repo
        let globals_path = temp_dir.path().join("bazel_features_globals");
        assert!(globals_path.exists());
        assert!(globals_path.join("globals.bzl").exists());
    }
}
