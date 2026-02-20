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
        // rules_rust internal extension
        ("//rust/private:extensions.bzl", "i")
        | ("@rules_rust//rust/private:extensions.bzl", "i") => {
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
    // For Bazel 9.0.0-kuro, we set values directly instead of using
    // getattr(native, ...) patterns, because starlark-rust resolves variable
    // references at compile time even in untaken ternary branches.
    let globals_content = r#"# Auto-generated globals for Kuro (Bazel 9.0 compatible)
# This file is generated by kuro_bzlmod::synthetic_repos
# All values are set directly for the emulated Bazel version.

globals = struct(
    # Available in Bazel 9.0:
    CcSharedLibraryHintInfo = None,  # 7.0.0+ but not a Kuro global yet
    PackageSpecificationInfo = None,  # 6.4.0+
    RunEnvironmentInfo = None,  # 5.3.0+
    set = None,  # 8.1.0+
    subrule = None,  # 7.0.0+
    DefaultInfo = DefaultInfo,  # always available
    macro = None,  # 8.0.0+
    # NOT available in Bazel 9.0:
    CcSharedLibraryInfo = None,
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

# buildifier: disable=bzl-visibility
load("@rules_rust//rust/private:rust.bzl", "rust_library_without_process_wrapper")
load("@rules_cc//cc:defs.bzl", "cc_library")

# Fake import macro impl stub - get_import_macro_deps checks label.name == "fake_import_macro_impl"
# to skip the import macro dependency. This avoids the complex select()/label_flag chain.
cc_library(
    name = "fake_import_macro_impl",
)

rust_library_without_process_wrapper(
    name = "tinyjson",
    srcs = ["src/lib.rs"],
    edition = "2018",
    visibility = ["//visibility:public"],
    _import_macro_dep = ":fake_import_macro_impl",
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
