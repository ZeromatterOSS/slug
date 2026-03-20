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
    collect_synthetic_repos_with_root(parsed_modules, None)
}

/// Collect synthetic repos with access to the project root for reading workspace files.
pub fn collect_synthetic_repos_with_root(
    parsed_modules: &[(String, ParsedModuleFile)],
    project_root: Option<&Path>,
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
            if let Some(synthetic) =
                generate_synthetic_repos_for_extension(usage, project_root)
            {
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
fn generate_synthetic_repos_for_extension(
    usage: &ExtensionUsage,
    project_root: Option<&Path>,
) -> Option<Vec<SyntheticRepo>> {
    // Match known extensions by their bzl file and name
    match (
        usage.extension_bzl_file.as_str(),
        usage.extension_name.as_str(),
    ) {
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
        // rules_java compatibility_proxy extension
        ("//java:rules_java_deps.bzl", "compatibility_proxy")
        | ("@rules_java//java:rules_java_deps.bzl", "compatibility_proxy") => {
            Some(generate_java_compatibility_proxy())
        }
        // rules_rust internal extension
        ("//rust/private:extensions.bzl", "i")
        | ("@rules_rust//rust/private:extensions.bzl", "i")
        | ("//rust/private:internal_extensions.bzl", "i")
        | ("@rules_rust//rust/private:internal_extensions.bzl", "i") => {
            Some(generate_rules_rust_internal_repos(usage))
        }
        // rules_rust cargo internal extension
        ("//cargo/private:internal_extensions.bzl", "i")
        | ("@rules_rust//cargo/private:internal_extensions.bzl", "i")
        // rules_rust crate_universe internal extensions
        | ("//crate_universe/private:internal_extensions.bzl", "i")
        | ("@rules_rust//crate_universe/private:internal_extensions.bzl", "i")
        // rules_rust test extensions
        | ("//test:test_extensions.bzl", _)
        | ("@rules_rust//test:test_extensions.bzl", _) => {
            Some(generate_rules_rust_internal_repos(usage))
        }
        // rules_python internal_deps extension (0.31.0)
        ("//python/private/bzlmod:internal_deps.bzl", "internal_deps")
        | ("@rules_python//python/private/bzlmod:internal_deps.bzl", "internal_deps")
        // rules_python config extension (1.8.0+)
        | ("//python/extensions:config.bzl", "config")
        | ("@rules_python//python/extensions:config.bzl", "config") => {
            Some(generate_rules_python_internal_repos(usage))
        }
        // rules_python python toolchain extension
        ("//python/extensions:python.bzl", "python")
        | ("@rules_python//python/extensions:python.bzl", "python") => {
            Some(generate_rules_python_toolchain_repos(usage))
        }
        // rules_rs / rules_rust crate universe extension
        ("//rs:extensions.bzl", "crate")
        | ("@rules_rs//rs:extensions.bzl", "crate")
        | ("//crate_universe:extensions.bzl", "crate")
        | ("@rules_rust//crate_universe:extensions.bzl", "crate") => {
            Some(generate_crate_universe_repos(usage, project_root))
        }
        // rules_rust rust toolchain extension
        ("//rust:extensions.bzl", "rust")
        | ("@rules_rust//rust:extensions.bzl", "rust") => {
            Some(generate_rules_rust_toolchain_repos(usage))
        }
        // LLVM/Clang toolchain extensions
        ("//extensions:toolchain.bzl", "toolchain")
        | ("@llvm//extensions:toolchain.bzl", "toolchain") => {
            Some(generate_llvm_toolchain_repos(usage))
        }
        _ => {
            // For unrecognized extensions, generate minimal stub repos for each use_repo name.
            // This prevents cell-not-found errors when projects use extensions we don't
            // natively support yet (e.g., rules_go, gazelle, aspect_bazel_lib, protobuf).
            let repos = generate_stub_repos_for_extension(usage);
            if repos.is_empty() {
                None
            } else {
                Some(repos)
            }
        }
    }
}

/// Generate minimal stub repos for an unrecognized module extension.
///
/// Each use_repo() name gets an empty BUILD file so the cell alias resolves.
/// The repos won't have real content, but this prevents resolution failures
/// for extensions whose repos are only transitively referenced.
fn generate_stub_repos_for_extension(usage: &ExtensionUsage) -> Vec<SyntheticRepo> {
    let mut repos = Vec::new();
    for import in &usage.imports {
        for repo_name in &import.repos {
            let mut files = HashMap::new();
            files.insert(
                "BUILD.bazel".to_owned(),
                format!(
                    "# Stub repo for unrecognized extension: {}:{}\nfilegroup(name = \"all\", srcs = glob([\"**\"]), visibility = [\"//visibility:public\"])\n",
                    usage.extension_bzl_file, usage.extension_name,
                ),
            );
            repos.push(SyntheticRepo {
                name: repo_name.clone(),
                files,
            });
        }
        // Also handle keyword remapping (apparent_name -> actual_name)
        for (apparent_name, _actual_name) in &import.repo_mapping {
            let mut files = HashMap::new();
            files.insert(
                "BUILD.bazel".to_owned(),
                format!(
                    "# Stub repo for unrecognized extension: {}:{}\nfilegroup(name = \"all\", srcs = glob([\"**\"]), visibility = [\"//visibility:public\"])\n",
                    usage.extension_bzl_file, usage.extension_name,
                ),
            );
            repos.push(SyntheticRepo {
                name: apparent_name.clone(),
                files,
            });
        }
    }
    repos
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

    // BUILD.bazel - exports the .bzl file and provides :version target for bzl_library deps
    files.insert(
        "BUILD.bazel".to_string(),
        r#"exports_files(["version.bzl"])

filegroup(
    name = "version",
    srcs = ["version.bzl"],
    visibility = ["//visibility:public"],
)
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
    // For Bazel 9.0.0-kuro, we set values directly instead of using
    // getattr(native, ...) patterns, because starlark-rust resolves variable
    // references at compile time even in untaken ternary branches.
    let globals_content = r#"# Auto-generated globals for Kuro (Bazel 9.0 compatible)
# This file is generated by kuro_bzlmod::synthetic_repos
# Globals reference real symbols where available, None otherwise.

globals = struct(
    # Available in Bazel 9.0 - reference real globals:
    CcSharedLibraryHintInfo = None,  # Removed before 9.0.0
    CcSharedLibraryInfo = CcSharedLibraryInfo,  # 6.0.0+ (real provider)
    DefaultInfo = DefaultInfo,  # always available
    PackageSpecificationInfo = PackageSpecificationInfo,  # 6.4.0+
    RunEnvironmentInfo = RunEnvironmentInfo,  # 5.3.0+ (real provider)
    macro = None,  # 8.0.0+ (disabled: our macro() doesn't handle attrs/defaults yet)
    set = None,  # 8.1.0+ (not yet available in starlark-rust)
    subrule = subrule,  # 7.0.0+ (real global function)
    # LEGACY globals - removed in Bazel 8.0+ (return None):
    cc_proto_aspect = None,
    JavaInfo = None,
    JavaPluginInfo = None,
    ProtoInfo = None,
    PyCcLinkParamsProvider = None,
    PyInfo = None,
    PyRuntimeInfo = None,
    __TestingOnly_NeverAvailable = None,
)
"#;

    files.insert("globals.bzl".to_string(), globals_content.to_string());

    // BUILD.bazel - exports the .bzl file and provides :globals target for bzl_library deps
    files.insert(
        "BUILD.bazel".to_string(),
        r#"exports_files(["globals.bzl"])

filegroup(
    name = "globals",
    srcs = ["globals.bzl"],
    visibility = ["//visibility:public"],
)
"#
        .to_string(),
    );

    SyntheticRepo {
        name: "bazel_features_globals".to_string(),
        files,
    }
}

/// Host platform information for toolchain configuration.
struct HostPlatformInfo {
    /// Bazel CPU identifier (e.g., "k8", "darwin_arm64", "x64_windows")
    cpu_name: &'static str,
    /// @platforms//os constraint value
    os_constraint: &'static str,
    /// @platforms//cpu constraint value
    cpu_constraint: &'static str,
    /// Compiler identifier (e.g., "gcc", "clang", "msvc")
    compiler: &'static str,
    /// Tool paths for cc_toolchain_config_info
    tool_paths: Vec<(&'static str, String)>,
}

/// Detect MSVC installation on Windows using vswhere.exe.
///
/// Returns the path to the MSVC bin directory (e.g.,
/// `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.41.34120\bin\Hostx64\x64`)
/// or None if not found.
#[cfg(target_os = "windows")]
fn detect_msvc_bin_dir(host_arch: &str) -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    use std::process::Command;

    // Try vswhere.exe to find VS installation
    let vswhere_paths = [
        "C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe",
        "C:\\Program Files\\Microsoft Visual Studio\\Installer\\vswhere.exe",
    ];

    let mut vs_install_path: Option<String> = None;
    for vswhere in &vswhere_paths {
        if let Ok(output) = Command::new(vswhere)
            .args([
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property",
                "installationPath",
            ])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    vs_install_path = Some(path);
                    break;
                }
            }
        }
    }

    let vs_path = vs_install_path?;
    let vc_tools = PathBuf::from(&vs_path).join("VC").join("Tools").join("MSVC");

    // Find the latest MSVC version directory
    let mut versions: Vec<_> = std::fs::read_dir(&vc_tools)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    versions.sort();
    let latest_version = versions.pop()?;

    let host_dir = match host_arch {
        "aarch64" => "Hostarm64",
        _ => "Hostx64",
    };
    let target_dir = match host_arch {
        "aarch64" => "arm64",
        _ => "x64",
    };

    let bin_dir = vc_tools
        .join(&latest_version)
        .join("bin")
        .join(host_dir)
        .join(target_dir);

    if bin_dir.exists() {
        Some(bin_dir)
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
fn detect_msvc_bin_dir(_host_arch: &str) -> Option<std::path::PathBuf> {
    None
}

fn detect_host_platform() -> HostPlatformInfo {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match (os, arch) {
        ("linux", "x86_64") => HostPlatformInfo {
            cpu_name: "k8",
            os_constraint: "@platforms//os:linux",
            cpu_constraint: "@platforms//cpu:x86_64",
            compiler: "gcc",
            tool_paths: vec![
                ("gcc", "/usr/bin/gcc".to_string()),
                ("ld", "/usr/bin/ld".to_string()),
                ("ar", "/usr/bin/ar".to_string()),
                ("cpp", "/usr/bin/cpp".to_string()),
                ("gcov", "/usr/bin/gcov".to_string()),
                ("nm", "/usr/bin/nm".to_string()),
                ("objdump", "/usr/bin/objdump".to_string()),
                ("strip", "/usr/bin/strip".to_string()),
            ],
        },
        ("linux", "aarch64") => HostPlatformInfo {
            cpu_name: "aarch64",
            os_constraint: "@platforms//os:linux",
            cpu_constraint: "@platforms//cpu:aarch64",
            compiler: "gcc",
            tool_paths: vec![
                ("gcc", "/usr/bin/gcc".to_string()),
                ("ld", "/usr/bin/ld".to_string()),
                ("ar", "/usr/bin/ar".to_string()),
                ("cpp", "/usr/bin/cpp".to_string()),
                ("gcov", "/usr/bin/gcov".to_string()),
                ("nm", "/usr/bin/nm".to_string()),
                ("objdump", "/usr/bin/objdump".to_string()),
                ("strip", "/usr/bin/strip".to_string()),
            ],
        },
        ("macos", "x86_64") => HostPlatformInfo {
            cpu_name: "darwin_x86_64",
            os_constraint: "@platforms//os:osx",
            cpu_constraint: "@platforms//cpu:x86_64",
            compiler: "clang",
            tool_paths: vec![
                ("gcc", "/usr/bin/clang".to_string()),
                ("ld", "/usr/bin/ld".to_string()),
                ("ar", "/usr/bin/ar".to_string()),
                ("cpp", "/usr/bin/clang".to_string()),
                ("gcov", "/usr/bin/gcov".to_string()),
                ("nm", "/usr/bin/nm".to_string()),
                ("objdump", "/usr/bin/objdump".to_string()),
                ("strip", "/usr/bin/strip".to_string()),
            ],
        },
        ("macos", "aarch64") => HostPlatformInfo {
            cpu_name: "darwin_arm64",
            os_constraint: "@platforms//os:osx",
            cpu_constraint: "@platforms//cpu:aarch64",
            compiler: "clang",
            tool_paths: vec![
                ("gcc", "/usr/bin/clang".to_string()),
                ("ld", "/usr/bin/ld".to_string()),
                ("ar", "/usr/bin/ar".to_string()),
                ("cpp", "/usr/bin/clang".to_string()),
                ("gcov", "/usr/bin/gcov".to_string()),
                ("nm", "/usr/bin/nm".to_string()),
                ("objdump", "/usr/bin/objdump".to_string()),
                ("strip", "/usr/bin/strip".to_string()),
            ],
        },
        ("windows", "x86_64") | ("windows", "aarch64") => {
            let (cpu_name, cpu_constraint) = if arch == "aarch64" {
                ("arm64_windows", "@platforms//cpu:aarch64")
            } else {
                ("x64_windows", "@platforms//cpu:x86_64")
            };

            // Auto-detect MSVC installation
            let msvc_bin = detect_msvc_bin_dir(arch);
            let (cl, link, lib, dumpbin) = if let Some(ref bin_dir) = msvc_bin {
                let base = bin_dir.to_string_lossy().to_string();
                (
                    format!("{}\\cl.exe", base),
                    format!("{}\\link.exe", base),
                    format!("{}\\lib.exe", base),
                    format!("{}\\dumpbin.exe", base),
                )
            } else {
                // Fallback to bare names (requires VS Developer Command Prompt)
                tracing::warn!("MSVC not detected via vswhere; using bare tool names. \
                    Ensure Visual Studio Build Tools are installed and cl.exe is on PATH.");
                (
                    "cl.exe".to_string(),
                    "link.exe".to_string(),
                    "lib.exe".to_string(),
                    "dumpbin.exe".to_string(),
                )
            };

            HostPlatformInfo {
                cpu_name,
                os_constraint: "@platforms//os:windows",
                cpu_constraint,
                compiler: "msvc-cl",
                tool_paths: vec![
                    ("gcc", cl.clone()),
                    ("ld", link),
                    ("ar", lib),
                    ("cpp", cl),
                    ("gcov", String::new()),
                    ("nm", dumpbin.clone()),
                    ("objdump", dumpbin),
                    ("strip", String::new()),
                ],
            }
        }
        // Fallback to Linux x86_64
        _ => HostPlatformInfo {
            cpu_name: "k8",
            os_constraint: "@platforms//os:linux",
            cpu_constraint: "@platforms//cpu:x86_64",
            compiler: "gcc",
            tool_paths: vec![
                ("gcc", "/usr/bin/gcc".to_string()),
                ("ld", "/usr/bin/ld".to_string()),
                ("ar", "/usr/bin/ar".to_string()),
                ("cpp", "/usr/bin/cpp".to_string()),
                ("gcov", "/usr/bin/gcov".to_string()),
                ("nm", "/usr/bin/nm".to_string()),
                ("objdump", "/usr/bin/objdump".to_string()),
                ("strip", "/usr/bin/strip".to_string()),
            ],
        },
    }
}

/// Generate the @local_config_cc and @local_config_cc_toolchains repos.
///
/// These are created by rules_cc's cc_configure_extension.
fn generate_rules_cc_repos() -> Vec<SyntheticRepo> {
    let host = detect_host_platform();
    vec![
        generate_local_config_cc_repo(&host),
        generate_local_config_cc_toolchains_repo(&host),
    ]
}

/// Generate the @local_config_cc repository.
///
/// This contains the detected C++ toolchain configuration.
fn generate_local_config_cc_repo(host: &HostPlatformInfo) -> SyntheticRepo {
    let mut files = HashMap::new();

    let cpu = host.cpu_name;

    // BUILD.bazel with basic toolchain detection
    // This is a simplified version - full implementation would detect system compiler
    // Note: We don't use package(default_visibility=...) because that's Bazel-specific
    // and doesn't work in Kuro's Buck2-based model. Instead, we set visibility explicitly.
    // Also, we use native cc_toolchain_suite and cc_toolchain instead of loading from
    // rules_cc, because the rules_cc versions have additional required implicit attributes.
    let build_content = format!(
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# This is a simplified toolchain configuration

load(":local_config.bzl", "local_config")

# Placeholder toolchain - actual detection happens at build time
cc_toolchain_suite(
    name = "toolchain",
    toolchains = {{
        "{cpu}": ":cc-compiler-{cpu}",
        "{cpu}|{compiler}": ":cc-compiler-{cpu}",
    }},
    visibility = ["//visibility:public"],
)

filegroup(
    name = "empty",
    srcs = [],
    visibility = ["//visibility:public"],
)

cc_toolchain(
    name = "cc-compiler-{cpu}",
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
"#,
        cpu = cpu,
        compiler = host.compiler,
    );

    files.insert("BUILD.bazel".to_string(), build_content);

    // Generate tool_paths entries for the .bzl file
    // Use forward slashes in paths for Starlark compatibility
    let tool_paths_str: String = host
        .tool_paths
        .iter()
        .filter(|(_, path)| !path.is_empty())
        .map(|(name, path)| {
            let normalized = path.replace('\\', "/");
            format!("        tool_path(name = \"{name}\", path = \"{normalized}\"),")
        })
        .collect::<Vec<_>>()
        .join("\n");

    // local_config.bzl - the rule definition must be in a .bzl file
    let bzl_content = format!(
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# Minimal toolchain configuration rule

load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "tool_path")

def _impl(ctx):
    return cc_common.create_cc_toolchain_config_info(
        ctx = ctx,
        toolchain_identifier = "local",
        host_system_name = "local",
        target_system_name = "local",
        target_cpu = "{cpu}",
        target_libc = "local",
        compiler = "{compiler}",
        abi_version = "local",
        abi_libc_version = "local",
        tool_paths = [
{tool_paths}
        ],
    )

local_config = rule(
    implementation = _impl,
    attrs = {{}},
    provides = [CcToolchainConfigInfo],
)
"#,
        cpu = cpu,
        compiler = host.compiler,
        tool_paths = tool_paths_str,
    );

    files.insert("local_config.bzl".to_string(), bzl_content.to_string());

    SyntheticRepo {
        name: "local_config_cc".to_string(),
        files,
    }
}

/// Generate the @local_config_cc_toolchains repository.
fn generate_local_config_cc_toolchains_repo(host: &HostPlatformInfo) -> SyntheticRepo {
    let mut files = HashMap::new();

    let cpu = host.cpu_name;
    let os_constraint = host.os_constraint;
    let cpu_constraint = host.cpu_constraint;

    // BUILD.bazel with toolchain registrations
    // Note: We don't use package(default_visibility=...) because that's Bazel-specific
    // and doesn't work in Kuro's Buck2-based model. Instead, we set visibility explicitly.
    let build_content = format!(
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos

toolchain(
    name = "cc-toolchain-{cpu}",
    exec_compatible_with = [
        "{cpu_constraint}",
        "{os_constraint}",
    ],
    target_compatible_with = [
        "{cpu_constraint}",
        "{os_constraint}",
    ],
    toolchain = "@local_config_cc//:cc-compiler-{cpu}",
    toolchain_type = "@rules_cc//cc:toolchain_type",
    visibility = ["//visibility:public"],
)

# Alias for :all pattern
alias(
    name = "all",
    actual = ":cc-toolchain-{cpu}",
    visibility = ["//visibility:public"],
)
"#,
        cpu = cpu,
        os_constraint = os_constraint,
        cpu_constraint = cpu_constraint,
    );

    files.insert("BUILD.bazel".to_string(), build_content);

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

/// Generate the @compatibility_proxy repository for rules_java.
///
/// Provides minimal stub implementations for JavaInfo, JavaPluginInfo, java_common, etc.
/// that allow BUILD files loading java_proto_library.bzl to succeed even when Java
/// compilation is not actually supported. Java targets will fail at analysis time but
/// other targets in the same BUILD file (e.g., python_toolchain) will load successfully.
fn generate_java_compatibility_proxy() -> Vec<SyntheticRepo> {
    let mut files = HashMap::new();

    files.insert(
        "BUILD.bazel".to_string(),
        r#"# Auto-generated by kuro - stub compatibility_proxy for rules_java
exports_files(["proxy.bzl"], visibility = ["//visibility:public"])
"#
        .to_string(),
    );

    // Minimal proxy.bzl that satisfies all loads from rules_java's compatibility_proxy
    let proxy_content = r#"# Auto-generated by kuro - stub compatibility_proxy for rules_java
# Provides minimal stubs needed for the Java proto build chain to load.

# JavaInfo provider stub
JavaInfo = provider(
    "JavaInfo",
    fields = {
        "compile_jars": "compile jars",
        "transitive_compile_time_jars": "transitive compile jars",
        "transitive_runtime_jars": "transitive runtime jars",
        "source_jars": "source jars",
    },
)

# JavaPluginInfo provider stub
JavaPluginInfo = provider(
    "JavaPluginInfo",
    fields = {
        "plugins": "plugins",
        "api_generating_plugins": "API generating plugins",
    },
)

# java_common stub module
java_common = struct(
    compile = None,
    merge = None,
)

# Internal compile/merge functions - None signals "use java_common fallback" in proto_support.bzl
java_common_internal_compile = None
java_info_internal_merge = None

# Stub rule implementations for Java rules (not needed for Python protos, but
# included so that any code loading them doesn't fail with symbol-not-found errors)
def _java_stub_impl(ctx):
    return [DefaultInfo()]

java_binary = rule(
    implementation = _java_stub_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(),
        "runtime_deps": attr.label_list(),
        "resources": attr.label_list(allow_files = True),
        "main_class": attr.string(),
        "javacopts": attr.string_list(),
        "plugins": attr.label_list(),
        "data": attr.label_list(allow_files = True),
    },
)

java_import = rule(
    implementation = _java_stub_impl,
    attrs = {
        "jars": attr.label_list(allow_files = True),
        "deps": attr.label_list(),
        "exports": attr.label_list(),
        "runtime_deps": attr.label_list(),
    },
)

java_library = rule(
    implementation = _java_stub_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(),
        "exports": attr.label_list(),
        "runtime_deps": attr.label_list(),
        "resources": attr.label_list(allow_files = True),
        "javacopts": attr.string_list(),
        "plugins": attr.label_list(),
        "data": attr.label_list(allow_files = True),
        "neverlink": attr.bool(),
    },
)

java_plugin = rule(
    implementation = _java_stub_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(),
        "processor_class": attr.string(),
        "generates_api": attr.bool(),
        "data": attr.label_list(allow_files = True),
        "neverlink": attr.bool(),
    },
)

java_test = rule(
    implementation = _java_stub_impl,
    test = True,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(),
        "runtime_deps": attr.label_list(),
        "resources": attr.label_list(allow_files = True),
        "javacopts": attr.string_list(),
        "plugins": attr.label_list(),
        "test_class": attr.string(),
        "data": attr.label_list(allow_files = True),
    },
)

java_package_configuration = rule(
    implementation = _java_stub_impl,
    attrs = {
        "packages": attr.label_list(),
        "javacopts": attr.string_list(),
    },
)

java_runtime = rule(
    implementation = _java_stub_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "java_home": attr.string(),
        "version": attr.int(),
    },
)

java_toolchain = rule(
    implementation = _java_stub_impl,
    attrs = {
        "source_version": attr.string(),
        "target_version": attr.string(),
        "java": attr.label(allow_single_file = True),
        "javabuilder": attr.label_list(),
    },
)

http_jar = rule(
    implementation = _java_stub_impl,
    attrs = {
        "urls": attr.string_list(),
        "sha256": attr.string(),
        "downloaded_file_name": attr.string(),
    },
)
"#;
    files.insert("proxy.bzl".to_string(), proxy_content.to_string());

    vec![SyntheticRepo {
        name: "compatibility_proxy".to_string(),
        files,
    }]
}

/// Generate synthetic repos for rules_rust's internal extension.
///
/// rules_rust's internal extension creates many repos via http_archive for
/// various optional features (bindgen, prost, wasm-bindgen, etc.).
/// Most of these are only needed for specific features; only `rules_rust_tinyjson`
/// is required for basic Rust compilation (it's used by process_wrapper).
///
/// We create stub repos for all use_repo entries so cell aliases resolve,
/// and provide actual content for repos needed during builds.
fn generate_rules_rust_internal_repos(usage: &ExtensionUsage) -> Vec<SyntheticRepo> {
    let mut repos = Vec::new();

    // Collect all repo names from use_repo() imports
    let mut all_repo_names = Vec::new();
    for import in &usage.imports {
        for repo in &import.repos {
            all_repo_names.push(repo.clone());
        }
        for (apparent, _actual) in &import.repo_mapping {
            all_repo_names.push(apparent.clone());
        }
    }

    for name in &all_repo_names {
        if name == "rules_rust_tinyjson" {
            repos.push(generate_rules_rust_tinyjson_repo());
        } else {
            // Create empty stub repo so the cell alias resolves
            let mut files = HashMap::new();
            files.insert(
                "BUILD.bazel".to_string(),
                format!(
                    "# Stub repo for rules_rust internal dependency: {}\n# This repo would be populated by rules_rust's module extension.\n",
                    name
                ),
            );
            repos.push(SyntheticRepo {
                name: name.clone(),
                files,
            });
        }
    }

    repos
}

/// Generate the @rules_rust_tinyjson repository.
///
/// This is a minimal stub that provides the `tinyjson` rust_library target
/// needed by rules_rust's process_wrapper. The actual tinyjson crate source
/// would normally be downloaded via http_archive.
fn generate_rules_rust_tinyjson_repo() -> SyntheticRepo {
    let mut files = HashMap::new();

    // BUILD.bazel - defines the tinyjson library target
    // process_wrapper depends on @rules_rust_tinyjson//:tinyjson
    // Must use _without_process_wrapper variant to break the cycle:
    //   tinyjson -> process_wrapper -> tinyjson
    let build_content = r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# Stub for tinyjson crate (normally downloaded from crates.io)
# Uses rust_library_without_process_wrapper to break the cycle:
#   process_wrapper -> tinyjson -> process_wrapper

load("@rules_rust//rust/private:rust.bzl", "rust_library_without_process_wrapper")

rust_library_without_process_wrapper(
    name = "tinyjson",
    srcs = ["src/lib.rs"],
    edition = "2018",
    visibility = ["//visibility:public"],
)
"#;
    files.insert("BUILD.bazel".to_string(), build_content.to_string());

    // Minimal tinyjson stub - provides the types that process_wrapper uses
    let lib_content = r#"//! Minimal tinyjson stub for kuro synthetic repos.
//! The real tinyjson crate provides JSON parsing/generation.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// JSON value type
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Number(f64),
    Boolean(bool),
    String(String),
    Null,
    Array(Vec<JsonValue>),
    Object(HashMap<String, JsonValue>),
}

impl JsonValue {
    pub fn get<S: AsRef<str>>(&self, key: S) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(map) => map.get(key.as_ref()),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }

    pub fn stringify(&self) -> Result<String, JsonGenerateError> {
        Ok(format!("{}", self))
    }
}

impl fmt::Display for JsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonValue::Number(n) => write!(f, "{}", n),
            JsonValue::Boolean(b) => write!(f, "{}", b),
            JsonValue::String(s) => write!(f, "\"{}\"", s),
            JsonValue::Null => write!(f, "null"),
            JsonValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            JsonValue::Object(map) => {
                write!(f, "{{")?;
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 { write!(f, ",")?; }
                    write!(f, "\"{}\":{}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// Error type for JSON parsing
#[derive(Debug)]
pub struct JsonParseError {
    msg: String,
}

impl fmt::Display for JsonParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON parse error: {}", self.msg)
    }
}

impl std::error::Error for JsonParseError {}

/// Error type for JSON generation
#[derive(Debug)]
pub struct JsonGenerateError;

impl fmt::Display for JsonGenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON generate error")
    }
}

impl std::error::Error for JsonGenerateError {}

impl FromStr for JsonValue {
    type Err = JsonParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_json(s.trim().as_bytes(), &mut 0)
    }
}

fn parse_json(bytes: &[u8], pos: &mut usize) -> Result<JsonValue, JsonParseError> {
    skip_whitespace(bytes, pos);
    if *pos >= bytes.len() {
        return Err(JsonParseError { msg: "unexpected end of input".to_string() });
    }
    match bytes[*pos] {
        b'"' => parse_string(bytes, pos).map(JsonValue::String),
        b'{' => parse_object(bytes, pos),
        b'[' => parse_array(bytes, pos),
        b't' | b'f' => parse_bool(bytes, pos),
        b'n' => parse_null(bytes, pos),
        b'-' | b'0'..=b'9' => parse_number(bytes, pos),
        c => Err(JsonParseError { msg: format!("unexpected char: {}", c as char) }),
    }
}

fn skip_whitespace(bytes: &[u8], pos: &mut usize) {
    while *pos < bytes.len() && matches!(bytes[*pos], b' ' | b'\t' | b'\n' | b'\r') {
        *pos += 1;
    }
}

fn parse_string(bytes: &[u8], pos: &mut usize) -> Result<String, JsonParseError> {
    *pos += 1; // skip opening quote
    let start = *pos;
    let mut result = String::new();
    while *pos < bytes.len() && bytes[*pos] != b'"' {
        if bytes[*pos] == b'\\' {
            *pos += 1;
            if *pos < bytes.len() {
                match bytes[*pos] {
                    b'"' => result.push('"'),
                    b'\\' => result.push('\\'),
                    b'/' => result.push('/'),
                    b'n' => result.push('\n'),
                    b'r' => result.push('\r'),
                    b't' => result.push('\t'),
                    _ => result.push(bytes[*pos] as char),
                }
            }
        } else {
            result.push(bytes[*pos] as char);
        }
        *pos += 1;
    }
    if *pos >= bytes.len() {
        return Err(JsonParseError { msg: "unterminated string".to_string() });
    }
    *pos += 1; // skip closing quote
    if result.is_empty() {
        Ok(String::from_utf8_lossy(&bytes[start..*pos - 1]).to_string())
    } else {
        Ok(result)
    }
}

fn parse_number(bytes: &[u8], pos: &mut usize) -> Result<JsonValue, JsonParseError> {
    let start = *pos;
    if bytes[*pos] == b'-' { *pos += 1; }
    while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
    if *pos < bytes.len() && bytes[*pos] == b'.' {
        *pos += 1;
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
    }
    if *pos < bytes.len() && (bytes[*pos] == b'e' || bytes[*pos] == b'E') {
        *pos += 1;
        if *pos < bytes.len() && (bytes[*pos] == b'+' || bytes[*pos] == b'-') { *pos += 1; }
        while *pos < bytes.len() && bytes[*pos].is_ascii_digit() { *pos += 1; }
    }
    let s = std::str::from_utf8(&bytes[start..*pos])
        .map_err(|_| JsonParseError { msg: "invalid number".to_string() })?;
    let n: f64 = s.parse().map_err(|_| JsonParseError { msg: "invalid number".to_string() })?;
    Ok(JsonValue::Number(n))
}

fn parse_bool(bytes: &[u8], pos: &mut usize) -> Result<JsonValue, JsonParseError> {
    if bytes[*pos..].starts_with(b"true") { *pos += 4; return Ok(JsonValue::Boolean(true)); }
    if bytes[*pos..].starts_with(b"false") { *pos += 5; return Ok(JsonValue::Boolean(false)); }
    Err(JsonParseError { msg: "expected bool".to_string() })
}

fn parse_null(bytes: &[u8], pos: &mut usize) -> Result<JsonValue, JsonParseError> {
    if bytes[*pos..].starts_with(b"null") { *pos += 4; return Ok(JsonValue::Null); }
    Err(JsonParseError { msg: "expected null".to_string() })
}

fn parse_object(bytes: &[u8], pos: &mut usize) -> Result<JsonValue, JsonParseError> {
    *pos += 1; // skip {
    let mut map = HashMap::new();
    skip_whitespace(bytes, pos);
    if *pos < bytes.len() && bytes[*pos] == b'}' { *pos += 1; return Ok(JsonValue::Object(map)); }
    loop {
        skip_whitespace(bytes, pos);
        let key = parse_string(bytes, pos)?;
        skip_whitespace(bytes, pos);
        if *pos >= bytes.len() || bytes[*pos] != b':' {
            return Err(JsonParseError { msg: "expected ':'".to_string() });
        }
        *pos += 1;
        let val = parse_json(bytes, pos)?;
        map.insert(key, val);
        skip_whitespace(bytes, pos);
        if *pos >= bytes.len() { return Err(JsonParseError { msg: "unterminated object".to_string() }); }
        if bytes[*pos] == b'}' { *pos += 1; return Ok(JsonValue::Object(map)); }
        if bytes[*pos] == b',' { *pos += 1; continue; }
        return Err(JsonParseError { msg: "expected ',' or '}'".to_string() });
    }
}

fn parse_array(bytes: &[u8], pos: &mut usize) -> Result<JsonValue, JsonParseError> {
    *pos += 1; // skip [
    let mut arr = Vec::new();
    skip_whitespace(bytes, pos);
    if *pos < bytes.len() && bytes[*pos] == b']' { *pos += 1; return Ok(JsonValue::Array(arr)); }
    loop {
        let val = parse_json(bytes, pos)?;
        arr.push(val);
        skip_whitespace(bytes, pos);
        if *pos >= bytes.len() { return Err(JsonParseError { msg: "unterminated array".to_string() }); }
        if bytes[*pos] == b']' { *pos += 1; return Ok(JsonValue::Array(arr)); }
        if bytes[*pos] == b',' { *pos += 1; continue; }
        return Err(JsonParseError { msg: "expected ',' or ']'".to_string() });
    }
}
"#;
    files.insert("src/lib.rs".to_string(), lib_content.to_string());

    SyntheticRepo {
        name: "rules_rust_tinyjson".to_string(),
        files,
    }
}

/// Generate synthetic repos for rules_python's internal_deps extension.
///
/// The internal_deps extension creates `rules_python_internal` which contains
/// `rules_python_config.bzl` with configuration flags. It also creates many
/// `pypi__*` repos for pip-related dependencies.
///
/// With `enable_pystar = True`, rules_python uses its Starlark implementations
/// (the Bazel 9.0 approach - no native.py_* fallback needed).
fn generate_rules_python_internal_repos(usage: &ExtensionUsage) -> Vec<SyntheticRepo> {
    let mut repos = Vec::new();

    // Generate rules_python_internal with config
    repos.push(generate_rules_python_internal_config_repo());

    // Generate stub repos for all other use_repo entries (pypi__* packages)
    for import in &usage.imports {
        for repo in &import.repos {
            if repo == "rules_python_internal" {
                continue; // Already generated above
            }
            let mut files = HashMap::new();
            files.insert(
                "BUILD.bazel".to_string(),
                format!(
                    "# Stub repo for rules_python internal dependency: {}\n# This repo would be populated by rules_python's internal_deps extension.\n",
                    repo
                ),
            );
            repos.push(SyntheticRepo {
                name: repo.clone(),
                files,
            });
        }
        for (apparent, _actual) in &import.repo_mapping {
            if apparent == "rules_python_internal" {
                continue;
            }
            let mut files = HashMap::new();
            files.insert(
                "BUILD.bazel".to_string(),
                format!(
                    "# Stub repo for rules_python internal dependency: {}\n",
                    apparent
                ),
            );
            repos.push(SyntheticRepo {
                name: apparent.clone(),
                files,
            });
        }
    }

    repos
}

/// Generate the @rules_python_internal repository.
///
/// This repo contains:
/// - `rules_python_config.bzl` with `config.enable_pystar = True`
///   (uses Starlark py_library/py_binary/py_test implementations)
/// - `py_internal.bzl` with py_internal_impl stubs
/// - BUILD file
fn generate_rules_python_internal_config_repo() -> SyntheticRepo {
    let mut files = HashMap::new();

    // rules_python_config.bzl - enable pystar (Starlark implementations)
    files.insert(
        "rules_python_config.bzl".to_string(),
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# enable_pystar = True -> use Starlark py_library/py_binary/py_test
config = struct(
  enable_pystar = True,
  enable_deprecation_warnings = False,
  bazel_9_or_later = True,
  # BuiltinPyInfo/BuiltinPyRuntimeInfo are the native provider references.
  # In Bazel, these come from the built-in Java providers.
  # In Kuro, PyInfo/PyRuntimeInfo are registered as globals.
  BuiltinPyInfo = PyInfo,
  BuiltinPyRuntimeInfo = PyRuntimeInfo,
  # Default value for --build_python_zip flag
  build_python_zip_default = "auto",
)
"#
        .to_string(),
    );

    // py_internal.bzl - stubs for pystar Starlark implementations
    files.insert(
        "py_internal.bzl".to_string(),
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# Stubs for rules_python internal APIs used by pystar implementations.
# These implement the _py_builtins methods that rules_python's Starlark
# code calls into (normally native Java code in Bazel).

def _get_label_repo_runfiles_path(label):
    """Returns the package path of a label within its repository's runfiles.

    This is used by rules_python to compute the Python import path prefix.
    For a label like `rules_pkg//pkg/private:manifest`, this returns "pkg/private".
    For a root-level label like `//foo:bar`, this returns "foo".
    """
    pkg = label.package if hasattr(label, "package") else ""
    return pkg

def _is_singleton_depset(files):
    """Optimized check for whether a depset has exactly one element."""
    return len(files.to_list()) == 1

def _get_legacy_external_runfiles(ctx):
    """Whether --legacy_external_runfiles is enabled."""
    return False

def _is_bzlmod_enabled(ctx):
    """Whether bzlmod is enabled."""
    return True

def _create_repo_mapping_manifest(ctx, runfiles, output):
    """Creates a repo mapping manifest file for bzlmod."""
    ctx.actions.write(output, "")

def _get_rule_name(ctx):
    """Returns the rule name (e.g., 'py_binary')."""
    return ctx.attr._rule_name if hasattr(ctx.attr, "_rule_name") else "py_library"

def _are_action_listeners_enabled(ctx):
    """Whether action listeners (extra actions) are enabled."""
    return False

def _add_py_extra_pseudo_action(ctx, dependency_transitive_python_sources):
    """No-op: extra actions are deprecated."""
    pass

def _merge_runfiles_with_generated_inits_empty_files_supplier(ctx, runfiles):
    """Adds auto-generated __init__.py files to runfiles."""
    return runfiles

def _copy_without_caching(ctx, read_from, write_to):
    """Copies a file with constant metadata (non-cacheable)."""
    ctx.actions.run_shell(
        inputs = [read_from],
        outputs = [write_to],
        command = "cp \"$1\" \"$2\"",
        arguments = [read_from.path, write_to.path],
    )

def _declare_constant_metadata_file(ctx, name, root):
    """Declares a file with constant metadata properties."""
    return ctx.actions.declare_file(name)

def _make_runfiles_respect_legacy_external_runfiles(ctx, runfiles):
    """Adjusts runfiles for legacy_external_runfiles flag."""
    return runfiles

def _expand_location_and_make_variables(ctx, attribute_name, expression, targets):
    """Expands $(location) and Make variables in a string."""
    return ctx.expand_location(expression, targets) if hasattr(ctx, "expand_location") else expression

def _is_tool_configuration(ctx):
    """Whether this is a tool/exec configuration (for host tools)."""
    return False

# Export as py_internal_impl struct (rules_python loads this name)
py_internal_impl = struct(
    get_label_repo_runfiles_path = _get_label_repo_runfiles_path,
    is_singleton_depset = _is_singleton_depset,
    get_legacy_external_runfiles = _get_legacy_external_runfiles,
    is_bzlmod_enabled = _is_bzlmod_enabled,
    create_repo_mapping_manifest = _create_repo_mapping_manifest,
    get_rule_name = _get_rule_name,
    are_action_listeners_enabled = _are_action_listeners_enabled,
    add_py_extra_pseudo_action = _add_py_extra_pseudo_action,
    merge_runfiles_with_generated_inits_empty_files_supplier = _merge_runfiles_with_generated_inits_empty_files_supplier,
    copy_without_caching = _copy_without_caching,
    declare_constant_metadata_file = _declare_constant_metadata_file,
    make_runfiles_respect_legacy_external_runfiles = _make_runfiles_respect_legacy_external_runfiles,
    expand_location_and_make_variables = _expand_location_and_make_variables,
    is_tool_configuration = _is_tool_configuration,
)
"#
        .to_string(),
    );

    // extra_transition_settings.bzl - empty list of extra transition settings (1.8.0+)
    files.insert(
        "extra_transition_settings.bzl".to_string(),
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# Extra transition settings for rules_python (empty by default).
# Users can add custom settings via config.add_transition_setting() in MODULE.bazel.
EXTRA_TRANSITION_SETTINGS = []
"#
        .to_string(),
    );

    // BUILD file
    files.insert(
        "BUILD.bazel".to_string(),
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# @rules_python_internal configuration repository

exports_files([
    "rules_python_config.bzl",
    "py_internal.bzl",
    "extra_transition_settings.bzl",
])
"#
        .to_string(),
    );

    SyntheticRepo {
        name: "rules_python_internal".to_string(),
        files,
    }
}

/// Generate synthetic repos for rules_python's python toolchain extension.
///
/// The python extension creates `pythons_hub` for Python toolchain registration.
/// We create a stub hub that allows `register_toolchains("@pythons_hub//:all")`
/// to succeed, and stub repos for any other use_repo entries.
fn generate_rules_python_toolchain_repos(usage: &ExtensionUsage) -> Vec<SyntheticRepo> {
    let mut repos = Vec::new();

    // Generate pythons_hub
    repos.push(generate_pythons_hub_repo());

    // Generate stubs for any other repos
    for import in &usage.imports {
        for repo in &import.repos {
            if repo == "pythons_hub" {
                continue; // Already generated above
            }
            let mut files = HashMap::new();
            files.insert(
                "BUILD.bazel".to_string(),
                format!("# Stub repo for rules_python toolchain: {}\n", repo),
            );
            repos.push(SyntheticRepo {
                name: repo.clone(),
                files,
            });
        }
    }

    repos
}

/// Generate the @pythons_hub repository.
///
/// This provides a stub toolchain hub for Python. The `:all` target is
/// referenced by `register_toolchains("@pythons_hub//:all")` in rules_python's
/// MODULE.bazel.
fn generate_pythons_hub_repo() -> SyntheticRepo {
    let mut files = HashMap::new();

    // BUILD.bazel with stub toolchain
    files.insert(
        "BUILD.bazel".to_string(),
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# Stub Python toolchain hub

# Empty filegroup for :all target (toolchain registration placeholder)
filegroup(
    name = "all",
    srcs = [],
    visibility = ["//visibility:public"],
)
"#
        .to_string(),
    );

    // versions.bzl - provides Python version information needed by config_settings
    files.insert(
        "versions.bzl".to_string(),
        r#"# Auto-generated by kuro_bzlmod::synthetic_repos
# Stub Python version information for rules_python config_settings

# Default Python version (system Python)
DEFAULT_PYTHON_VERSION = "3.11"

# Minor version mapping (e.g., "3.11" -> "3.11.0")
MINOR_MAPPING = {
    "3.11": "3.11.0",
}

# List of all available Python versions (full versions, not minor)
PYTHON_VERSIONS = ["3.11.0"]
"#
        .to_string(),
    );

    SyntheticRepo {
        name: "pythons_hub".to_string(),
        files,
    }
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
        fs::create_dir_all(&repo_path).with_context(|| {
            format!("Failed to create synthetic repo directory: {:?}", repo_path)
        })?;

        for (file_path, content) in &repo.files {
            let full_path = repo_path.join(file_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            // Skip writing if content is unchanged to avoid spurious mtime updates.
            // Mtime changes trigger "File changed" cache invalidation on every build.
            let should_write = match fs::read_to_string(&full_path) {
                Ok(existing) => existing != *content,
                Err(_) => true,
            };
            if should_write {
                let mut file = fs::File::create(&full_path)
                    .with_context(|| format!("Failed to create file: {:?}", full_path))?;
                file.write_all(content.as_bytes())?;
            }
        }

        paths.push(repo_path);
    }

    Ok(paths)
}

/// Generate crate universe repos for rules_rs/rules_rust crate extension.
///
/// Creates the hub repo (e.g., `crates`) with `defs.bzl` containing
/// `all_crate_deps()` and related functions, plus a `BUILD.bazel` with
/// alias targets for each external crate.
///
/// If project_root is available, parses Cargo.lock to generate real crate lists.
/// Otherwise falls back to stub functions that return empty lists.
fn generate_crate_universe_repos(
    usage: &ExtensionUsage,
    project_root: Option<&Path>,
) -> Vec<SyntheticRepo> {
    let mut repos = Vec::new();

    // Find the hub repo name from from_cargo() tag or use_repo()
    let mut hub_name = None;
    for tag in &usage.tags {
        if tag.tag_name == "from_cargo" {
            for (k, v) in &tag.kwargs {
                if k == "name" {
                    if let crate::types::TagValue::String(s) = v {
                        hub_name = Some(s.clone());
                    }
                }
            }
        }
    }
    // Fallback: use the first use_repo name
    if hub_name.is_none() {
        for import in &usage.imports {
            if let Some(first) = import.repos.first() {
                hub_name = Some(first.clone());
                break;
            }
        }
    }

    let hub_name = match hub_name {
        Some(n) => n,
        None => return repos,
    };

    // Try to find Cargo.lock and Cargo.toml paths from the from_cargo() tag
    let mut cargo_lock_label = String::new();
    let mut cargo_toml_label = String::new();
    for tag in &usage.tags {
        if tag.tag_name == "from_cargo" {
            for (k, v) in &tag.kwargs {
                let s = match v {
                    crate::types::TagValue::String(s) | crate::types::TagValue::Label(s) => {
                        Some(s.clone())
                    }
                    _ => None,
                };
                if let Some(s) = s {
                    match k.as_str() {
                        "cargo_lock" => cargo_lock_label = s,
                        "cargo_toml" => cargo_toml_label = s,
                        _ => {}
                    }
                }
            }
        }
    }

    // Parse crate data from Cargo.lock if project root is available
    let crate_data = if let Some(root) = project_root {
        let lock_path = label_to_path(root, &cargo_lock_label);
        let toml_path = label_to_path(root, &cargo_toml_label);
        parse_cargo_crate_data(&lock_path, &toml_path)
    } else {
        None
    };

    // Generate hub repo
    let mut files = HashMap::new();

    let (defs_content, build_content) = match &crate_data {
        Some(data) => generate_crate_hub_files(data, &hub_name),
        None => generate_crate_hub_stubs(),
    };

    files.insert("defs.bzl".to_owned(), defs_content);
    files.insert("BUILD.bazel".to_owned(), build_content);

    repos.push(SyntheticRepo {
        name: hub_name.clone(),
        files,
    });

    // Generate spoke repos for each external crate
    if let Some(data) = &crate_data {
        for ext_crate in &data.external_crates {
            let spoke_name = format!(
                "{}__{}",
                hub_name,
                spoke_repo_name(&ext_crate.name, &ext_crate.version)
            );
            let mut files = HashMap::new();
            files.insert(
                "BUILD.bazel".to_owned(),
                generate_spoke_build_file(&ext_crate.name),
            );
            repos.push(SyntheticRepo {
                name: spoke_name,
                files,
            });
        }

        // Also generate the crate_index repo (used by some rules_rs versions)
        let mut index_files = HashMap::new();
        index_files.insert(
            "BUILD.bazel".to_owned(),
            "# Crate index stub\n".to_owned(),
        );
        repos.push(SyntheticRepo {
            name: "crate_index".to_owned(),
            files: index_files,
        });
    }

    // Generate any additional use_repo names not covered above
    for import in &usage.imports {
        for repo_name in &import.repos {
            if repo_name != &hub_name
                && !repos.iter().any(|r| r.name == *repo_name)
            {
                let mut files = HashMap::new();
                files.insert(
                    "BUILD.bazel".to_owned(),
                    format!(
                        "# Stub for crate extension repo: {}\n\
                        filegroup(name = \"all\", srcs = glob([\"**\"]), visibility = [\"//visibility:public\"])\n",
                        repo_name
                    ),
                );
                repos.push(SyntheticRepo {
                    name: repo_name.clone(),
                    files,
                });
            }
        }
    }

    repos
}

/// Convert a Bazel label like "//:Cargo.lock" to a filesystem path.
fn label_to_path(project_root: &Path, label: &str) -> PathBuf {
    // Strip leading "//:" or "//" prefix
    let relative = label
        .strip_prefix("//:")
        .or_else(|| label.strip_prefix("//"))
        .unwrap_or(label);
    project_root.join(relative)
}

/// Spoke repo name format matching crate_universe conventions.
fn spoke_repo_name(crate_name: &str, version: &str) -> String {
    format!("{}-{}", crate_name, version)
}

/// Data parsed from Cargo.lock and Cargo.toml.
struct CrateData {
    /// External crates (from crates.io or git)
    external_crates: Vec<ExternalCrate>,
    /// Mapping from workspace member package path to its dependencies
    member_deps: HashMap<String, MemberDeps>,
}

struct ExternalCrate {
    name: String,
    version: String,
}

#[derive(Clone)]
struct MemberDeps {
    normal: Vec<String>,
    dev: Vec<String>,
    build: Vec<String>,
}

/// Parse Cargo.lock and workspace Cargo.toml to extract crate dependency data.
fn parse_cargo_crate_data(lock_path: &Path, toml_path: &Path) -> Option<CrateData> {
    let lock_content = fs::read_to_string(lock_path).ok()?;
    let toml_content = fs::read_to_string(toml_path).ok()?;

    // Parse workspace members from Cargo.toml
    let workspace_members = parse_workspace_members(&toml_content);

    // Parse Cargo.lock to get all packages
    let (local_packages, external_crates) = parse_cargo_lock(&lock_content);

    // Build member deps by reading each member's Cargo.toml
    let project_root = toml_path.parent()?;
    let mut member_deps = HashMap::new();

    for member_path in &workspace_members {
        let member_toml_path = project_root.join(member_path).join("Cargo.toml");
        if let Ok(member_toml) = fs::read_to_string(&member_toml_path) {
            if let Some(pkg_name) = parse_package_name(&member_toml) {
                let deps = parse_member_dependencies(&member_toml, &local_packages);
                member_deps.insert(member_path.clone(), deps.clone());
                // Also index by package name for lookup
                member_deps.insert(pkg_name, deps);
            }
        }
    }

    Some(CrateData {
        external_crates,
        member_deps,
    })
}

/// Parse workspace members from Cargo.toml.
fn parse_workspace_members(content: &str) -> Vec<String> {
    let mut members = Vec::new();
    let mut in_members = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("members") && trimmed.contains('[') {
            in_members = true;
            // Check for inline single-line array
            if trimmed.contains(']') {
                // Single-line: members = ["a", "b"]
                if let Some(start) = trimmed.find('[') {
                    if let Some(end) = trimmed.find(']') {
                        let items = &trimmed[start + 1..end];
                        for item in items.split(',') {
                            let item = item.trim().trim_matches('"').trim_matches('\'');
                            if !item.is_empty() {
                                members.push(item.to_owned());
                            }
                        }
                    }
                }
                in_members = false;
                continue;
            }
            continue;
        }
        if in_members {
            if trimmed == "]" {
                in_members = false;
                continue;
            }
            let item = trimmed.trim_matches(',').trim().trim_matches('"').trim_matches('\'');
            if !item.is_empty() && !item.starts_with('#') {
                members.push(item.to_owned());
            }
        }
    }

    members
}

/// Parse package name from a Cargo.toml.
fn parse_package_name(content: &str) -> Option<String> {
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') && trimmed != "[package]" {
            in_package = false;
            continue;
        }
        if in_package && trimmed.starts_with("name") {
            if let Some(eq_pos) = trimmed.find('=') {
                let value = trimmed[eq_pos + 1..].trim().trim_matches('"').trim_matches('\'');
                return Some(value.to_owned());
            }
        }
    }
    None
}

/// Parse Cargo.lock to extract local and external packages.
fn parse_cargo_lock(content: &str) -> (std::collections::HashSet<String>, Vec<ExternalCrate>) {
    let mut local_packages = std::collections::HashSet::new();
    let mut external_crates = Vec::new();

    let mut current_name = String::new();
    let mut current_version = String::new();
    let mut current_source: Option<String> = None;
    let mut in_package = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            // Save previous package
            if in_package && !current_name.is_empty() {
                if current_source.is_some() {
                    external_crates.push(ExternalCrate {
                        name: current_name.clone(),
                        version: current_version.clone(),
                    });
                } else {
                    local_packages.insert(current_name.clone());
                }
            }
            in_package = true;
            current_name.clear();
            current_version.clear();
            current_source = None;
            continue;
        }

        if in_package {
            if trimmed.starts_with("name = ") {
                current_name = trimmed
                    .strip_prefix("name = ")
                    .unwrap_or("")
                    .trim_matches('"')
                    .to_owned();
            } else if trimmed.starts_with("version = ") {
                current_version = trimmed
                    .strip_prefix("version = ")
                    .unwrap_or("")
                    .trim_matches('"')
                    .to_owned();
            } else if trimmed.starts_with("source = ") {
                current_source = Some(
                    trimmed
                        .strip_prefix("source = ")
                        .unwrap_or("")
                        .trim_matches('"')
                        .to_owned(),
                );
            }
        }
    }

    // Don't forget the last package
    if in_package && !current_name.is_empty() {
        if current_source.is_some() {
            external_crates.push(ExternalCrate {
                name: current_name,
                version: current_version,
            });
        } else {
            local_packages.insert(current_name);
        }
    }

    // Deduplicate external crates (keep first occurrence of each name)
    let mut seen = std::collections::HashSet::new();
    external_crates.retain(|c| seen.insert(c.name.clone()));

    (local_packages, external_crates)
}

/// Parse dependencies from a workspace member's Cargo.toml.
/// Returns only external crate names (filters out local workspace deps).
fn parse_member_dependencies(
    content: &str,
    local_packages: &std::collections::HashSet<String>,
) -> MemberDeps {
    let mut normal = Vec::new();
    let mut dev = Vec::new();
    let mut build = Vec::new();

    let mut current_section = "";

    for line in content.lines() {
        let trimmed = line.trim();

        // Track section headers
        if trimmed.starts_with('[') {
            if trimmed == "[dependencies]" {
                current_section = "normal";
            } else if trimmed == "[dev-dependencies]" {
                current_section = "dev";
            } else if trimmed == "[build-dependencies]" {
                current_section = "build";
            } else if trimmed.starts_with("[dependencies.")
                || trimmed.starts_with("[dev-dependencies.")
                || trimmed.starts_with("[build-dependencies.")
            {
                // Detailed dependency section like [dependencies.serde]
                // Keep current section
            } else {
                current_section = "";
            }
            continue;
        }

        // Parse dependency lines in known sections
        if !current_section.is_empty() && trimmed.contains('=') && !trimmed.starts_with('#') {
            if let Some(dep_name) = trimmed.split('=').next() {
                let dep_name = dep_name.trim();
                // Skip if it's a local workspace package
                if !local_packages.contains(dep_name) && !dep_name.contains('.') {
                    let target = match current_section {
                        "normal" => &mut normal,
                        "dev" => &mut dev,
                        "build" => &mut build,
                        _ => continue,
                    };
                    target.push(dep_name.to_owned());
                }
            }
        }
    }

    MemberDeps { normal, dev, build }
}

/// Generate the hub repo's defs.bzl and BUILD.bazel with real crate data.
fn generate_crate_hub_files(data: &CrateData, hub_name: &str) -> (String, String) {
    // Generate defs.bzl
    let mut defs = String::new();
    defs.push_str("# Auto-generated by kuro crate universe resolver\n\n");

    // Build the dependency mapping as a Starlark dict
    defs.push_str("_NORMAL_DEPS = {\n");
    for (member_path, deps) in &data.member_deps {
        if !deps.normal.is_empty() {
            defs.push_str(&format!("    \"{}\": [\n", member_path));
            for dep in &deps.normal {
                defs.push_str(&format!(
                    "        \"@{}//:{}\",\n",
                    hub_name,
                    dep.replace('-', "_")
                ));
            }
            defs.push_str("    ],\n");
        }
    }
    defs.push_str("}\n\n");

    defs.push_str("_NORMAL_DEV_DEPS = {\n");
    for (member_path, deps) in &data.member_deps {
        if !deps.dev.is_empty() {
            defs.push_str(&format!("    \"{}\": [\n", member_path));
            for dep in &deps.dev {
                defs.push_str(&format!(
                    "        \"@{}//:{}\",\n",
                    hub_name,
                    dep.replace('-', "_")
                ));
            }
            defs.push_str("    ],\n");
        }
    }
    defs.push_str("}\n\n");

    defs.push_str("_BUILD_DEPS = {\n");
    for (member_path, deps) in &data.member_deps {
        if !deps.build.is_empty() {
            defs.push_str(&format!("    \"{}\": [\n", member_path));
            for dep in &deps.build {
                defs.push_str(&format!(
                    "        \"@{}//:{}\",\n",
                    hub_name,
                    dep.replace('-', "_")
                ));
            }
            defs.push_str("    ],\n");
        }
    }
    defs.push_str("}\n\n");

    // all_crate_deps function
    defs.push_str(r#"def all_crate_deps(
        normal = False,
        normal_dev = False,
        build = False,
        build_proc_macro = False,
        proc_macro = False,
        package_name = None):
    """Returns all crate dependencies for the calling package."""
    pkg = package_name if package_name else native.package_name()
    deps = []
    if normal:
        deps = deps + _NORMAL_DEPS.get(pkg, [])
    if normal_dev:
        deps = deps + _NORMAL_DEV_DEPS.get(pkg, [])
    if build or build_proc_macro:
        deps = deps + _BUILD_DEPS.get(pkg, [])
    if proc_macro:
        pass  # proc macros included in normal deps
    return deps

def aliases(**kwargs):
    """Returns crate aliases (empty for now)."""
    return {}

def crate_deps(deps, **kwargs):
    """Returns labels for specific crate names."""
    return deps

def crate_repositories():
    """No-op for bzlmod."""
    pass
"#);

    // Generate BUILD.bazel with alias targets
    let mut build = String::new();
    build.push_str("# Auto-generated by kuro crate universe resolver\n\n");
    build.push_str("package(default_visibility = [\"//visibility:public\"])\n\n");

    for ext_crate in &data.external_crates {
        let underscored = ext_crate.name.replace('-', "_");
        let spoke = spoke_repo_name(&ext_crate.name, &ext_crate.version);
        // Use original crate name (with hyphens) as the primary target name,
        // matching Bazel's crate_universe behavior
        build.push_str(&format!(
            "alias(name = \"{}\", actual = \"@{}__{}//:{}\", visibility = [\"//visibility:public\"])\n",
            ext_crate.name, hub_name, spoke, underscored,
        ));
        // Also generate an underscored alias if different, for compatibility
        if underscored != ext_crate.name {
            build.push_str(&format!(
                "alias(name = \"{}\", actual = \"@{}__{}//:{}\", visibility = [\"//visibility:public\"])\n",
                underscored, hub_name, spoke, underscored,
            ));
        }
    }

    (defs, build)
}

/// Generate stub hub files when Cargo.lock is not available.
fn generate_crate_hub_stubs() -> (String, String) {
    let defs = r#"# Stub crate universe defs (Cargo.lock not available)

def all_crate_deps(**kwargs):
    """Returns empty deps (stub implementation)."""
    return []

def aliases(**kwargs):
    return {}

def crate_deps(deps, **kwargs):
    return deps

def crate_repositories():
    pass
"#
    .to_owned();

    let build = "# Stub crate hub\npackage(default_visibility = [\"//visibility:public\"])\n"
        .to_owned();

    (defs, build)
}

/// Generate a spoke repo's BUILD.bazel for a single external crate.
fn generate_spoke_build_file(crate_name: &str) -> String {
    let target_name = crate_name.replace('-', "_");
    format!(
        r#"# Stub for crate: {crate_name}
# Full implementation requires downloading and compiling crate source.
package(default_visibility = ["//visibility:public"])

filegroup(
    name = "{target_name}",
    srcs = [],
    visibility = ["//visibility:public"],
)
"#,
        crate_name = crate_name,
        target_name = target_name,
    )
}

/// Generate rules_rust toolchain repos.
fn generate_rules_rust_toolchain_repos(usage: &ExtensionUsage) -> Vec<SyntheticRepo> {
    let mut repos = Vec::new();

    // Generate stub repos for each use_repo name
    for import in &usage.imports {
        for repo_name in &import.repos {
            let mut files = HashMap::new();

            if repo_name == "rust_toolchains" {
                // The main toolchain registration repo
                files.insert(
                    "BUILD.bazel".to_owned(),
                    r#"# Rust toolchain registration stub
package(default_visibility = ["//visibility:public"])

toolchain_type(name = "rustc", visibility = ["//visibility:public"])
toolchain_type(name = "rustfmt", visibility = ["//visibility:public"])
toolchain_type(name = "rust_analyzer", visibility = ["//visibility:public"])
"#
                    .to_owned(),
                );
            } else {
                files.insert(
                    "BUILD.bazel".to_owned(),
                    format!(
                        "# Rust toolchain stub: {}\npackage(default_visibility = [\"//visibility:public\"])\n",
                        repo_name
                    ),
                );
            }

            repos.push(SyntheticRepo {
                name: repo_name.clone(),
                files,
            });
        }
    }

    repos
}

/// Generate LLVM toolchain repos.
fn generate_llvm_toolchain_repos(usage: &ExtensionUsage) -> Vec<SyntheticRepo> {
    let mut repos = Vec::new();

    for import in &usage.imports {
        for repo_name in &import.repos {
            let mut files = HashMap::new();

            if repo_name == "llvm_toolchains" {
                files.insert(
                    "BUILD.bazel".to_owned(),
                    r#"# LLVM toolchain registration stub
package(default_visibility = ["//visibility:public"])

# Placeholder - actual LLVM toolchain setup requires downloading LLVM
filegroup(name = "all", srcs = [], visibility = ["//visibility:public"])
"#
                    .to_owned(),
                );
            } else {
                files.insert(
                    "BUILD.bazel".to_owned(),
                    format!(
                        "# LLVM toolchain stub: {}\npackage(default_visibility = [\"//visibility:public\"])\n",
                        repo_name
                    ),
                );
            }

            repos.push(SyntheticRepo {
                name: repo_name.clone(),
                files,
            });
        }
    }

    repos
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
            module: crate::types::Module::new(
                "test".to_string(),
                crate::version::Version::parse("1.0.0").unwrap(),
            ),
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
