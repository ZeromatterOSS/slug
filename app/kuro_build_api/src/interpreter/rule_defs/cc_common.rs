/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bazel-compatible cc_common module and CcInfo provider.
//!
//! This provides an implementation of Bazel's cc_common built-in module
//! that rules_cc (0.2.16+) requires for C/C++ compilation support.
//!
//! For Bazel 9.0+, rules_cc is almost entirely pure Starlark. The key native
//! requirement is this cc_common module which provides:
//! - `internal_DO_NOT_USE()` - Returns internal API struct
//! - Public API functions for toolchain/action configuration
//!
//! Reference: thoughts/shared/research/2026-01-26-rules-cc-native-requirements.md

use std::fmt;
use std::fmt::Display;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::OnceLock;

use allocative::Allocative;
use kuro_core::provider::id::ProviderId;
use kuro_interpreter::types::provider::callable::ProviderCallableLike;
use kuro_util::late_binding::LateBinding;
use starlark::coerce::Coerce;
use starlark::collections::SmallMap;
use starlark::collections::StarlarkHasher;
use starlark::environment::GlobalsBuilder;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::eval::Arguments;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::FreezeResult;
use starlark::values::Freezer;
use starlark::values::FrozenValue;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::dict::Dict;
use starlark::values::dict::DictRef;
use starlark::values::list::AllocList;
use starlark::values::none::NoneOr;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

use crate::interpreter::rule_defs::context::AnalysisActions;
use crate::interpreter::rule_defs::fragments::ConfigurationFragments;
use crate::interpreter::rule_defs::provider::ProviderLike;

/// Global storage for include directories discovered during analysis.
/// Populated when cc_common.compile() processes sources and strip_include_prefix,
/// and when native cc_library stubs are created for external repos.
/// Used by create_cc_compile_action to add -I flags.
static EXTERNAL_INCLUDE_DIRS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Register an include directory path for use in compile actions.
/// The path should be a relative path like "external/protobuf/src/" or "external/abseil-cpp/".
pub fn register_external_include_dir(include_dir: &str) {
    if let Ok(mut dirs) = EXTERNAL_INCLUDE_DIRS.lock() {
        if !dirs.iter().any(|d| d == include_dir) {
            dirs.push(include_dir.to_owned());
        }
    }
}

/// Get all registered external include directories.
pub fn get_external_include_dirs() -> Vec<String> {
    EXTERNAL_INCLUDE_DIRS
        .lock()
        .map(|dirs| dirs.clone())
        .unwrap_or_default()
}

/// Detect whether the compiler is MSVC (cl.exe) based on the compiler path.
fn is_msvc_compiler(compiler_path: &str) -> bool {
    let lower = compiler_path.to_lowercase();
    lower == "cl.exe"
        || lower == "cl"
        || lower.ends_with("\\cl.exe")
        || lower.ends_with("/cl.exe")
}

/// Returns true if the host OS is Windows.
fn is_windows_host() -> bool {
    std::env::consts::OS == "windows"
}

/// Cached MSVC tool paths detected via vswhere.
/// Maps tool name ("cl.exe", "link.exe", "lib.exe") to full path.
static MSVC_TOOL_CACHE: std::sync::OnceLock<Option<MsvcToolPaths>> = std::sync::OnceLock::new();

struct MsvcToolPaths {
    cl: String,
    link: String,
    lib: String,
    /// MSVC standard library include dir (e.g., .../MSVC/14.41/include)
    msvc_include: String,
    /// Windows SDK ucrt include dir
    ucrt_include: String,
    /// Windows SDK um include dir
    um_include: String,
    /// Windows SDK shared include dir
    shared_include: String,
    /// Windows SDK ucrt lib dir
    ucrt_lib: String,
    /// Windows SDK um lib dir
    um_lib: String,
    /// MSVC lib dir
    msvc_lib: String,
}

/// Detect and cache MSVC tool paths on Windows.
fn get_msvc_tool_paths() -> &'static Option<MsvcToolPaths> {
    MSVC_TOOL_CACHE.get_or_init(|| {
        #[cfg(target_os = "windows")]
        {
            use std::path::PathBuf;
            use std::process::Command;

            let vswhere_paths = [
                "C:\\Program Files (x86)\\Microsoft Visual Studio\\Installer\\vswhere.exe",
                "C:\\Program Files\\Microsoft Visual Studio\\Installer\\vswhere.exe",
            ];

            let mut vs_path_opt: Option<String> = None;
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
                            vs_path_opt = Some(path);
                            break;
                        }
                    }
                }
            }

            let vs_path = vs_path_opt?;
            let vc_tools = PathBuf::from(&vs_path).join("VC").join("Tools").join("MSVC");

            let mut versions: Vec<String> = std::fs::read_dir(&vc_tools)
                .ok()?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect();
            versions.sort();
            let msvc_ver = versions.pop()?;

            let msvc_base = vc_tools.join(&msvc_ver);
            let bin_dir = msvc_base.join("bin").join("Hostx64").join("x64");
            if !bin_dir.exists() {
                return None;
            }

            let msvc_include = msvc_base.join("include");
            let msvc_lib = msvc_base.join("lib").join("x64");

            // Find Windows SDK
            let sdk_root = PathBuf::from("C:\\Program Files (x86)\\Windows Kits\\10");
            let sdk_include = sdk_root.join("Include");
            let sdk_lib = sdk_root.join("Lib");

            // Find latest SDK version
            let sdk_ver = std::fs::read_dir(&sdk_include)
                .ok()
                .and_then(|entries| {
                    let mut vers: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .filter(|n| n.starts_with("10."))
                        .collect();
                    vers.sort();
                    vers.pop()
                })
                .unwrap_or_default();

            let base = bin_dir.to_string_lossy().to_string();
            let ucrt_inc = sdk_include.join(&sdk_ver).join("ucrt");
            let um_inc = sdk_include.join(&sdk_ver).join("um");
            let shared_inc = sdk_include.join(&sdk_ver).join("shared");
            let ucrt_lib_dir = sdk_lib.join(&sdk_ver).join("ucrt").join("x64");
            let um_lib_dir = sdk_lib.join(&sdk_ver).join("um").join("x64");

            Some(MsvcToolPaths {
                cl: format!("{}\\cl.exe", base),
                link: format!("{}\\link.exe", base),
                lib: format!("{}\\lib.exe", base),
                msvc_include: msvc_include.to_string_lossy().to_string(),
                ucrt_include: ucrt_inc.to_string_lossy().to_string(),
                um_include: um_inc.to_string_lossy().to_string(),
                shared_include: shared_inc.to_string_lossy().to_string(),
                ucrt_lib: ucrt_lib_dir.to_string_lossy().to_string(),
                um_lib: um_lib_dir.to_string_lossy().to_string(),
                msvc_lib: msvc_lib.to_string_lossy().to_string(),
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    })
}

/// Resolve a Windows compiler path. If bare "cl.exe", try to find the full MSVC path.
fn resolve_windows_compiler(bare_path: &str) -> String {
    if bare_path == "cl.exe" || bare_path == "cl" {
        if let Some(tools) = get_msvc_tool_paths() {
            return tools.cl.clone();
        }
    }
    bare_path.to_owned()
}

/// Choose the appropriate include flag for a directory path.
///
/// - Repo roots (`external/repo`) use `-isystem` (searched before standard system dirs)
/// - One level below repo root (`external/repo/src`) use `-isystem` too — needed for
///   repos that use `strip_include_prefix = "/src"` (like protobuf).
/// - Deep subdirs (`external/repo/a/b/...`, depth>=2) use `-idirafter` (searched AFTER
///   standard system dirs) to prevent files like `endian.h` in `absl/base/internal/` from
///   shadowing `/usr/include/endian.h`
/// - Non-external dirs use `-I`
///
/// IMPORTANT: Uses non-empty path segment counting to correctly handle trailing slashes.
/// e.g. `external/protobuf/src/` has depth=1 (one segment "src"), NOT depth=2.
fn include_flag_for_dir(dir: &str) -> String {
    include_flag_for_dir_impl(dir, is_windows_host())
}

fn include_flag_for_dir_impl(dir: &str, msvc: bool) -> String {
    // MSVC uses /I for all include types (no -isystem/-idirafter distinction)
    if msvc {
        return format!("/I{}", dir);
    }
    if dir.starts_with("external/") || dir.starts_with("bazel-out/") {
        // Count non-empty path components after "external/<repo>/"
        if dir.starts_with("external/") {
            if let Some(second_slash) = dir[9..].find('/') {
                let after_repo = &dir[9 + second_slash..];
                // Count non-empty segments to handle trailing slashes correctly.
                // e.g. "/src/" has 1 non-empty segment ("src"), not 2 slashes.
                let depth = after_repo.split('/').filter(|s| !s.is_empty()).count();
                if depth >= 2 {
                    // Deep subdir: use -idirafter to avoid shadowing system headers
                    return format!("-idirafter{}", dir);
                }
            }
        }
        format!("-isystem{}", dir)
    } else {
        format!("-I{}", dir)
    }
}

/// Normalize a `buck-out/v2/external_cells/bzlmod/<name>/<version>/...` path to
/// the equivalent `external/<name>/...` path for include path computation.
///
/// This is needed because source artifacts from external bzlmod cells use the
/// full `buck-out/v2/external_cells/` path, but `external/<name>` is a symlink
/// to the same location and is the canonical form for include paths.
fn normalize_external_cells_path(path: &str) -> Option<String> {
    let prefix = "buck-out/v2/external_cells/bzlmod/";
    if !path.starts_with(prefix) {
        return None;
    }
    let rest = &path[prefix.len()..];
    // rest = "<name>/<version>/..."
    let name_end = rest.find('/')?;
    let name = &rest[..name_end];
    let after_name = &rest[name_end + 1..];
    // Skip the version component
    let version_end = after_name.find('/')?;
    let after_version = &after_name[version_end + 1..];
    Some(format!("external/{}/{}", name, after_version))
}

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
    enabled_features: Vec<String>,
}

/// Default features that are typically enabled by CC toolchains.
/// These match common Bazel CC toolchain defaults.
fn default_cc_features() -> Vec<&'static str> {
    let mut features = vec![
        // Core features always enabled
        "supports_dynamic_linker",
        "supports_interface_shared_libraries",
        "supports_start_end_lib",
        "static_link_cpp_runtimes",
        "compiler_param_file",
        "linker_param_file",
        // Compilation modes
        "fastbuild",
        "dbg",
        "opt",
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
impl<'v> StarlarkValue<'v> for FeatureConfiguration {}

// ============================================================================
// CcCompilationContext - Compilation context for C++ builds
// ============================================================================

/// CcCompilationContext holds the compilation context (headers, includes, defines).
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct CcCompilationContextGen<V: ValueLifetimeless> {
    /// Headers depset
    headers: V,
    /// Include directories (generic -I)
    includes: V,
    /// Quote include directories (-iquote)
    quote_includes: V,
    /// System include directories (-isystem)
    system_includes: V,
    /// Framework include directories (-F)
    framework_includes: V,
    /// Defines
    defines: V,
    /// Local defines (not propagated to dependents)
    local_defines: V,
}

starlark_complex_value!(pub CcCompilationContext);

impl<V: ValueLifetimeless> Display for CcCompilationContextGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<CcCompilationContext>")
    }
}

#[starlark::values::starlark_value(type = "CcCompilationContext")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcCompilationContextGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "headers"
                | "includes"
                | "quote_includes"
                | "system_includes"
                | "framework_includes"
                | "external_includes"
                | "defines"
                | "local_defines"
                | "direct_headers"
                | "direct_public_headers"
                | "direct_private_headers"
                | "direct_textual_headers"
                | "validation_artifacts"
                | "_header_info"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "headers" => {
                if self.headers.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.headers.to_value())
                }
            }
            "includes" => {
                if self.includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.includes.to_value())
                }
            }
            "quote_includes" => {
                if self.quote_includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.quote_includes.to_value())
                }
            }
            "system_includes" | "external_includes" => {
                if self.system_includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.system_includes.to_value())
                }
            }
            "framework_includes" => {
                if self.framework_includes.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.framework_includes.to_value())
                }
            }
            "defines" => {
                if self.defines.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.defines.to_value())
                }
            }
            "local_defines" => {
                if self.local_defines.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.local_defines.to_value())
                }
            }
            "direct_headers"
            | "direct_public_headers"
            | "direct_private_headers"
            | "direct_textual_headers" => Some(heap.alloc(AllocList::EMPTY)),
            "validation_artifacts" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_header_info" => Some(heap.alloc(HeaderInfoStub)),
            _ => None,
        }
    }
}

// ============================================================================
// CcToolchainVariables - Variables for C++ toolchain configuration
// ============================================================================

/// CcToolchainVariables holds build variables for C++ toolchain configuration.
///
/// Used by cc_common functions to pass configuration to compile/link actions.
/// This version stores a reference to the original variables dict for access
/// by get_link_args.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct CcToolchainVariablesGen<V: ValueLifetimeless> {
    /// The original variables dict
    pub(crate) vars: V,
}

starlark_complex_value!(pub CcToolchainVariables);

impl<V: ValueLifetimeless> Display for CcToolchainVariablesGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcToolchainVariables()")
    }
}

#[starlark::values::starlark_value(type = "CcToolchainVariables")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcToolchainVariablesGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        // Allow attribute access to underlying variables dict
        let vars_value = self.vars.to_value();
        if vars_value.is_none() {
            return None;
        }
        // Use DictRef to access dict values by string key
        if let Some(dict_ref) = DictRef::from_value(vars_value) {
            dict_ref.get_str(attribute)
        } else {
            None
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        let vars_value = self.vars.to_value();
        if vars_value.is_none() {
            return false;
        }
        if let Some(dict_ref) = DictRef::from_value(vars_value) {
            dict_ref.get_str(attribute).is_some()
        } else {
            false
        }
    }
}

// ============================================================================
// CtxCheatStub - Stub for actions2ctx_cheat return value
// ============================================================================

/// A stub context returned by actions2ctx_cheat (used when no real actions available).
///
/// This provides the minimum attributes needed by rules_cc's compile function.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatStub;

impl Display for CtxCheatStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx_cheat_stub>")
    }
}

starlark_simple_value!(CtxCheatStub);

#[starlark_value(type = "ctx_cheat_stub")]
impl<'v> StarlarkValue<'v> for CtxCheatStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "label"
                | "bin_dir"
                | "genfiles_dir"
                | "configuration"
                | "actions"
                | "fragments"
                | "workspace_name"
                | "exec_groups"
                | "toolchains"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "label" => Some(heap.alloc(CtxCheatLabelStub)),
            "bin_dir" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(CtxCheatDirStub {
                    path: format!("bazel-out/{}-{}/bin", crate::interpreter::rule_defs::context::host_target_cpu(), m),
                }))
            },
            "genfiles_dir" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(CtxCheatDirStub {
                    path: format!("bazel-out/{}-{}/genfiles", crate::interpreter::rule_defs::context::host_target_cpu(), m),
                }))
            },
            "configuration" => Some(heap.alloc(CtxCheatConfigStub)),
            "actions" => Some(heap.alloc(CtxCheatActionsStub)),
            "fragments" => {
                let mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                let cpp = crate::interpreter::rule_defs::fragments::CppFragment::new(
                    mode, false, false, false,
                );
                Some(heap.alloc(ConfigurationFragments::new(cpp)))
            },
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            "exec_groups" => {
                Some(heap.alloc(crate::interpreter::rule_defs::context::ExecGroupsDict))
            }
            "toolchains" => {
                Some(heap.alloc(crate::interpreter::rule_defs::context::ToolchainsStub))
            }
            _ => None,
        }
    }
}

/// A context wrapper returned by actions2ctx_cheat that preserves the real actions.
///
/// This wraps the real AnalysisActions so that create_cc_compile_action can
/// use them to register actual compile actions.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative, Trace)]
pub struct CtxCheatWithActions<'v> {
    /// The real actions object (AnalysisActions)
    actions: Value<'v>,
    /// Target cell name (e.g., "protobuf")
    #[allocative(skip)]
    cell_name: String,
    /// Package path (e.g., "third_party/utf8_range")
    #[allocative(skip)]
    pkg_path: String,
    /// Target name (e.g., "utf8_validity")
    #[allocative(skip)]
    target_name: String,
}

impl<'v> Display for CtxCheatWithActions<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx_cheat_with_actions>")
    }
}

#[starlark_value(type = "ctx_cheat_stub")]
impl<'v> StarlarkValue<'v> for CtxCheatWithActions<'v> {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "label"
                | "bin_dir"
                | "genfiles_dir"
                | "configuration"
                | "actions"
                | "fragments"
                | "workspace_name"
                | "exec_groups"
                | "toolchains"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "label" => Some(heap.alloc(CtxCheatLabelDynamic {
                name: self.target_name.clone(),
                package: self.pkg_path.clone(),
                workspace_name: self.cell_name.clone(),
            })),
            "bin_dir" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(CtxCheatDirStub {
                    path: format!("bazel-out/{}-{}/bin", crate::interpreter::rule_defs::context::host_target_cpu(), m),
                }))
            },
            "genfiles_dir" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(CtxCheatDirStub {
                    path: format!("bazel-out/{}-{}/genfiles", crate::interpreter::rule_defs::context::host_target_cpu(), m),
                }))
            },
            "configuration" => Some(heap.alloc(CtxCheatConfigStub)),
            // Return the REAL actions object here
            "actions" => Some(self.actions),
            "fragments" => {
                let mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                let cpp = crate::interpreter::rule_defs::fragments::CppFragment::new(
                    mode, false, false, false,
                );
                Some(heap.alloc(ConfigurationFragments::new(cpp)))
            },
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            "exec_groups" => {
                Some(heap.alloc(crate::interpreter::rule_defs::context::ExecGroupsDict))
            }
            "toolchains" => {
                Some(heap.alloc(crate::interpreter::rule_defs::context::ToolchainsStub))
            }
            _ => None,
        }
    }
}

impl<'v> starlark::values::AllocValue<'v> for CtxCheatWithActions<'v> {
    fn alloc_value(self, heap: Heap<'v>) -> Value<'v> {
        heap.alloc_complex_no_freeze(self)
    }
}

/// A stub for ctx.actions.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatActionsStub;

impl Display for CtxCheatActionsStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<actions>")
    }
}

starlark_simple_value!(CtxCheatActionsStub);

#[starlark_value(type = "actions")]
impl<'v> StarlarkValue<'v> for CtxCheatActionsStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_actions_stub_methods)
    }
}

#[starlark_module]
fn ctx_cheat_actions_stub_methods(builder: &mut MethodsBuilder) {
    /// Declares a file in the output tree.
    #[allow(unused_variables)]
    fn declare_file<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = NoneType)] sibling: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Return a stub artifact
        Ok(heap.alloc(CtxCheatArtifactStub {
            path: filename.to_owned(),
        }))
    }

    /// Declares a directory in the output tree.
    #[allow(unused_variables)]
    fn declare_directory<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = pos)] filename: &str,
        #[starlark(require = named, default = NoneType)] sibling: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CtxCheatArtifactStub {
            path: filename.to_owned(),
        }))
    }

    /// Runs an action (stub implementation).
    #[allow(unused_variables)]
    fn run<'v>(
        this: &CtxCheatActionsStub,
        #[starlark(require = named, default = NoneType)] mnemonic: Value<'v>,
        #[starlark(require = named, default = NoneType)] executable: Value<'v>,
        #[starlark(require = named, default = NoneType)] arguments: Value<'v>,
        #[starlark(require = named, default = NoneType)] inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] progress_message: Value<'v>,
        #[starlark(require = named, default = NoneType)] resource_set: Value<'v>,
        #[starlark(require = named, default = NoneType)] env: Value<'v>,
        #[starlark(require = named, default = false)] use_default_shell_env: bool,
        #[starlark(require = named, default = NoneType)] execution_requirements: Value<'v>,
        #[starlark(require = named, default = NoneType)] toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] exec_group: Value<'v>,
        #[starlark(require = named, default = NoneType)] tools: Value<'v>,
        #[starlark(require = named, default = NoneType)] input_manifests: Value<'v>,
        #[starlark(require = named, default = NoneType)] unused_inputs_list: Value<'v>,
        #[starlark(require = named, default = NoneType)] shadowed_action: Value<'v>,
    ) -> starlark::Result<NoneType> {
        // Stub: do nothing - just accept the parameters
        Ok(NoneType)
    }
}

/// A stub for artifact root (Bazel compatibility).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatArtifactRootStub {
    path: String,
}

impl Display for CtxCheatArtifactRootStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<root {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatArtifactRootStub);

#[starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for CtxCheatArtifactRootStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        attribute == "path"
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

/// A stub for artifact from ctx.actions.declare_file.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatArtifactStub {
    path: String,
}

impl Display for CtxCheatArtifactStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<artifact {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatArtifactStub);

#[starlark_value(type = "File")]
impl<'v> StarlarkValue<'v> for CtxCheatArtifactStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "path"
                | "short_path"
                | "basename"
                | "extension"
                | "is_source"
                | "root"
                | "is_directory"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            "short_path" => Some(heap.alloc_str(&self.path).to_value()),
            "basename" => {
                let basename = self.path.rsplit('/').next().unwrap_or(&self.path);
                Some(heap.alloc_str(basename).to_value())
            }
            "extension" => {
                let ext = self.path.rsplit('.').next().unwrap_or("");
                Some(heap.alloc_str(ext).to_value())
            }
            "is_source" => Some(Value::new_bool(false)),
            "is_directory" => Some(Value::new_bool(false)),
            "root" => {
                let m = crate::interpreter::rule_defs::build_config::get_compilation_mode();
                Some(heap.alloc(CtxCheatArtifactRootStub {
                    path: format!("bazel-out/{}-{}/bin", crate::interpreter::rule_defs::context::host_target_cpu(), m),
                }))
            },
            _ => None,
        }
    }

    fn equals(&self, other: Value<'v>) -> starlark::Result<bool> {
        match CtxCheatArtifactStub::from_value(other) {
            Some(other) => Ok(self.path == other.path),
            None => Ok(false),
        }
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> starlark::Result<()> {
        self.path.hash(hasher);
        Ok(())
    }
}

/// A stub for ctx.configuration.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatConfigStub;

impl Display for CtxCheatConfigStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<configuration>")
    }
}

starlark_simple_value!(CtxCheatConfigStub);

#[starlark_value(type = "configuration")]
impl<'v> StarlarkValue<'v> for CtxCheatConfigStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_config_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "coverage_enabled")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "coverage_enabled" => Some(Value::new_bool(false)),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_cheat_config_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns whether sibling repository layout is used.
    fn is_sibling_repository_layout(this: &CtxCheatConfigStub) -> starlark::Result<bool> {
        let _ = this;
        Ok(false)
    }
}

/// A stub for directory paths (bin_dir, genfiles_dir).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatDirStub {
    path: String,
}

impl Display for CtxCheatDirStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<dir {}>", self.path)
    }
}

starlark_simple_value!(CtxCheatDirStub);

#[starlark_value(type = "root")]
impl<'v> StarlarkValue<'v> for CtxCheatDirStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "path")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "path" => Some(heap.alloc_str(&self.path).to_value()),
            _ => None,
        }
    }
}

/// A stub label for the ctx_cheat_stub.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatLabelStub;

impl Display for CtxCheatLabelStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "//stub:stub")
    }
}

starlark_simple_value!(CtxCheatLabelStub);

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for CtxCheatLabelStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_label_stub_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "name" | "package" | "workspace_name")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc_str("stub").to_value()),
            "package" => Some(heap.alloc_str("stub").to_value()),
            "workspace_name" => Some(heap.alloc_str("").to_value()),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_cheat_label_stub_methods(builder: &mut MethodsBuilder) {
    /// Returns a label with the same package but a different name.
    fn same_package_label<'v>(
        this: &CtxCheatLabelStub,
        #[starlark(require = pos)] name: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return a new label stub with the given name
        Ok(heap.alloc(CtxCheatLabelStub))
    }
}

/// A dynamic label with real target info for the ctx_cheat.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CtxCheatLabelDynamic {
    name: String,
    package: String,
    workspace_name: String,
}

impl Display for CtxCheatLabelDynamic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.workspace_name.is_empty() {
            write!(f, "//{}:{}", self.package, self.name)
        } else {
            write!(
                f,
                "@{}//{}:{}",
                self.workspace_name, self.package, self.name
            )
        }
    }
}

starlark_simple_value!(CtxCheatLabelDynamic);

#[starlark_value(type = "Label")]
impl<'v> StarlarkValue<'v> for CtxCheatLabelDynamic {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(ctx_cheat_label_dynamic_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "name" | "package" | "workspace_name")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "name" => Some(heap.alloc_str(&self.name).to_value()),
            "package" => Some(heap.alloc_str(&self.package).to_value()),
            "workspace_name" => Some(heap.alloc_str(&self.workspace_name).to_value()),
            _ => None,
        }
    }
}

#[starlark_module]
fn ctx_cheat_label_dynamic_methods(builder: &mut MethodsBuilder) {
    /// Returns a label with the same package but a different name.
    fn same_package_label<'v>(
        this: &CtxCheatLabelDynamic,
        #[starlark(require = pos)] name: &str,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CtxCheatLabelDynamic {
            name: name.to_owned(),
            package: this.package.clone(),
            workspace_name: this.workspace_name.clone(),
        }))
    }
}

// ============================================================================
// HeaderInfoStub - Stub for header info returned by create_header_info
// ============================================================================

/// A stub for HeaderInfo returned by create_header_info.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct HeaderInfoStub;

impl Display for HeaderInfoStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<HeaderInfo>")
    }
}

starlark_simple_value!(HeaderInfoStub);

#[starlark_value(type = "HeaderInfo")]
impl<'v> StarlarkValue<'v> for HeaderInfoStub {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "modular_public_headers"
                | "modular_private_headers"
                | "textual_headers"
                | "separate_module_headers"
                | "header_module"
                | "pic_header_module"
                | "separate_module"
                | "separate_pic_module"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "modular_public_headers"
            | "modular_private_headers"
            | "textual_headers"
            | "separate_module_headers" => {
                // Return empty list
                Some(heap.alloc(AllocList::EMPTY))
            }
            "header_module" | "pic_header_module" | "separate_module" | "separate_pic_module" => {
                // Return None
                Some(Value::new_none())
            }
            _ => None,
        }
    }
}

// ============================================================================
// CcCommonInternal - Internal API returned by internal_DO_NOT_USE()
// ============================================================================

/// Helper: push a path string or its corresponding artifact to the args list.
fn push_path_or_artifact<'v>(
    path_str: &str,
    artifact_map: &std::collections::HashMap<String, Value<'v>>,
    args: &mut Vec<Value<'v>>,
    heap: Heap<'v>,
) {
    if let Some(&artifact) = artifact_map.get(path_str) {
        args.push(artifact);
    } else {
        args.push(heap.alloc_str(path_str).to_value());
    }
}

/// Internal cc_common API struct.
///
/// Returned by `cc_common.internal_DO_NOT_USE()`. Contains internal functions
/// that rules_cc uses for low-level C++ compilation actions.
///
/// Reference: cc/private/cc_internal.bzl in rules_cc
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcCommonInternal;

impl Display for CcCommonInternal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cc_common.internal")
    }
}

starlark_simple_value!(CcCommonInternal);

#[starlark_value(type = "cc_common_internal")]
impl<'v> StarlarkValue<'v> for CcCommonInternal {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_common_internal_methods)
    }
}

/// Internal methods for cc_common.internal_DO_NOT_USE() return value.
///
/// These are used by rules_cc's internal Starlark code.
#[starlark_module]
fn cc_common_internal_methods(builder: &mut MethodsBuilder) {
    /// Creates a C++ compile action.
    ///
    /// This is a native function that registers a compile action with Kuro's
    /// action execution system. It bridges rules_cc's Starlark code to the
    /// native action registration infrastructure.
    #[allow(unused_variables)]
    fn create_cc_compile_action<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] action_construction_context: Value<'v>,
        #[starlark(require = named, default = NoneType)] cc_compilation_context: Value<'v>,
        #[starlark(require = named, default = NoneType)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] copts_filter: Value<'v>,
        #[starlark(require = named, default = NoneType)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_compilation_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_include_scanning_roots: Value<
            'v,
        >,
        #[starlark(require = named, default = NoneType)] source: Value<'v>,
        #[starlark(require = named, default = NoneType)] output_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] diagnostics_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] dotd_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] gcno_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] dwo_file: Value<'v>,
        #[starlark(require = named, default = false)] use_pic: bool,
        #[starlark(require = named, default = NoneType)] lto_indexing_file: Value<'v>,
        #[starlark(require = named)] action_name: NoneOr<&str>,
        #[starlark(require = named, default = NoneType)] compile_build_variables: Value<'v>,
        #[starlark(require = named, default = false)] needs_include_validation: bool,
        #[starlark(require = named, default = NoneType)] toolchain_type: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        let heap = eval.heap();

        // Log call for debugging
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[create_cc_compile_action] source={}, output_file={}, action_name={:?}",
                source, output_file, action_name
            );
            let _ = writeln!(
                f,
                "  action_construction_context type: {}",
                action_construction_context.get_type()
            );
        }

        // Validate required parameters
        if source.is_none() || output_file.is_none() {
            // Cannot create compile action without source and output
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cc_common_compile.log")
            {
                let _ = writeln!(f, "  EARLY RETURN: source or output_file is None");
            }
            return Ok(NoneType);
        }

        // Get the actions from action_construction_context
        // The context is a CtxCheatWithActions that has the real actions
        let actions_attr_result = action_construction_context.get_attr("actions", heap);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "  actions attr result: {:?}",
                actions_attr_result
                    .as_ref()
                    .map(|o| o.map(|v| v.to_string()))
            );
        }
        let actions_value = if let Ok(Some(actions)) = actions_attr_result {
            actions
        } else {
            // Fallback: action_construction_context might itself be actions
            action_construction_context
        };
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(f, "  actions_value type: {}", actions_value.get_type());
        }

        // Try to get the run method from actions
        let run_attr_result = actions_value.get_attr("run", heap);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "  run attr result: {:?}",
                run_attr_result.as_ref().map(|o| o.map(|v| v.to_string()))
            );
        }
        let run_method = match run_attr_result {
            Ok(Some(method)) => method,
            _ => {
                // No run method available - this is a stub context
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(f, "  EARLY RETURN: no run method available");
                }
                return Ok(NoneType);
            }
        };

        // Get source path for progress message
        let source_path = source
            .get_attr("path", heap)
            .ok()
            .flatten()
            .and_then(|v| v.unpack_str())
            .unwrap_or("unknown")
            .to_owned();

        // Get the action name for mnemonic/category
        // Convert Bazel action names (with hyphens) to Kuro categories (snake_case)
        let action_name_raw = action_name.into_option().unwrap_or("c-compile");
        let action_name_str = action_name_raw.replace("-", "_");

        // Determine if this is a C++ compile action (vs plain C)
        let is_cpp = action_name_raw.contains("c++") || action_name_raw.contains("cpp");

        // Get compiler path from toolchain if available, otherwise use platform default
        let default_compiler = match std::env::consts::OS {
            "windows" => {
                // On Windows, resolve cl.exe to its full MSVC path
                if let Some(tools) = get_msvc_tool_paths() {
                    tools.cl.as_str()
                } else {
                    "cl.exe"
                }
            }
            "macos" => "/usr/bin/clang++",
            _ => if is_cpp { "/usr/bin/g++" } else { "/usr/bin/gcc" },
        };
        let compiler_path = if !cc_toolchain.is_none() {
            // Try to get compiler path from toolchain
            let raw = cc_toolchain
                .get_attr("compiler_executable", heap)
                .ok()
                .flatten()
                .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
                .unwrap_or_else(|| default_compiler.to_owned());
            // Resolve bare "cl.exe" to full path on Windows
            if is_windows_host() { resolve_windows_compiler(&raw) } else { raw }
        } else {
            default_compiler.to_owned()
        };

        // Need to call .as_output() on the output artifact to mark it as an output
        // This is required by Kuro's run() to bind the artifact to an action
        let output_artifact = match output_file.get_attr("as_output", heap) {
            Ok(Some(as_output_method)) => eval
                .eval_function(as_output_method, &[], &[])
                .unwrap_or(output_file),
            _ => output_file,
        };

        // Build the command line arguments list
        let msvc = is_msvc_compiler(&compiler_path);
        let mut args_vec: Vec<Value<'v>> = Vec::new();

        // Get output path as string for MSVC /Fo flag
        let output_path_str = output_file
            .get_attr("path", heap)
            .ok()
            .flatten()
            .and_then(|v| v.unpack_str().map(|s| s.to_owned()))
            .unwrap_or_default();

        if msvc {
            // MSVC flags: cl.exe /nologo /EHsc /c source /Fo<output>
            args_vec.push(heap.alloc_str(&compiler_path).to_value());
            args_vec.push(heap.alloc_str("/nologo").to_value());
            args_vec.push(heap.alloc_str("/EHsc").to_value());
            args_vec.push(heap.alloc_str("/c").to_value());
            args_vec.push(source);
            args_vec.push(heap.alloc_str(&format!("/Fo{}", output_path_str)).to_value());

            // Add MSVC system include paths (STL headers, Windows SDK)
            if let Some(tools) = get_msvc_tool_paths() {
                for inc in [&tools.msvc_include, &tools.ucrt_include, &tools.um_include, &tools.shared_include] {
                    if !inc.is_empty() {
                        args_vec.push(heap.alloc_str(&format!("/I{}", inc)).to_value());
                    }
                }
            }
        } else {
            // GCC/Clang flags: -c source -o output -fPIC
            args_vec.push(heap.alloc_str("-c").to_value());
            args_vec.push(source);
            args_vec.push(heap.alloc_str("-o").to_value());
            args_vec.push(output_artifact);
            // -fPIC for position-independent code (not applicable to MSVC)
            args_vec.push(heap.alloc_str("-fPIC").to_value());
        }

        // Add include directories from compilation context (deduplicated)
        let mut seen_include_dirs = std::collections::HashSet::new();
        if !cc_compilation_context.is_none() {
            for attr_name in &["includes", "system_includes", "quote_includes"] {
                if let Ok(Some(includes_val)) = cc_compilation_context.get_attr(attr_name, heap) {
                    if !includes_val.is_none() {
                        let mut elements = Vec::new();
                        crate::interpreter::rule_defs::depset::collect_depset_elements(
                            includes_val,
                            &mut elements,
                            heap,
                        );
                        for elem in &elements {
                            let dir = elem.to_str();
                            if dir.is_empty()
                                || dir.contains("_virtual_includes")
                                || !seen_include_dirs.insert(dir.to_string())
                            {
                                continue;
                            }
                            let flag = include_flag_for_dir_impl(&dir, msvc);
                            args_vec.push(heap.alloc_str(&flag).to_value());
                        }
                    }
                }
            }
        }

        // Add include paths for external repos and source directories.
        if let Some(src_path_str) = source
            .get_attr("path", heap)
            .ok()
            .flatten()
            .and_then(|v| v.unpack_str())
        {
            // Normalize buck-out/v2/external_cells/bzlmod/<name>/<version>/... paths to
            // external/<name>/... for include path computation. This ensures that the same
            // include path logic applies whether the source is referenced via the symlink
            // (external/<name>/...) or the raw buck-out path.
            let normalized_src_path;
            let effective_src_path: &str =
                if let Some(norm) = normalize_external_cells_path(src_path_str) {
                    normalized_src_path = norm;
                    &normalized_src_path
                } else {
                    src_path_str
                };

            // For external repo sources with /src/ dir, add as include path
            if let Some(ext_idx) = effective_src_path.find("/src/") {
                let inc_dir = &effective_src_path[..ext_idx + 5];
                if seen_include_dirs.insert(inc_dir.to_string()) {
                    let flag = include_flag_for_dir_impl(inc_dir, msvc);
                    args_vec.push(heap.alloc_str(&flag).to_value());
                }
                register_external_include_dir(inc_dir);
            }
            // Also add "external/<repo>/" for direct includes and "external/" for
            // repo-name-prefixed includes (e.g., `#include "rules_cc/cc/..."` in
            // rules_cc source files).
            if effective_src_path.starts_with("external/") {
                if let Some(second_slash) = effective_src_path[9..].find('/') {
                    let repo_dir = &effective_src_path[..9 + second_slash];
                    if seen_include_dirs.insert(repo_dir.to_string()) {
                        let flag = include_flag_for_dir_impl(repo_dir, msvc);
                        args_vec.push(heap.alloc_str(&flag).to_value());
                    }
                    register_external_include_dir(repo_dir);
                }
                if seen_include_dirs.insert("external/".to_owned()) {
                    let ext_flag = if msvc { "/Iexternal/" } else { "-isystemexternal/" };
                    args_vec.push(heap.alloc_str(ext_flag).to_value());
                }
                register_external_include_dir("external/");
            }
            // Register source file's parent directory as an include path.
            // Uses -idirafter (via include_flag_for_dir) for deep paths to avoid
            // shadowing system headers.
            if effective_src_path.starts_with("external/") {
                if let Some(second_slash) = effective_src_path[9..].find('/') {
                    let repo_end = 9 + second_slash;
                    if let Some(last_slash) = effective_src_path.rfind('/') {
                        let src_dir = &effective_src_path[..last_slash];
                        let depth = src_dir[repo_end..].matches('/').count();
                        if depth >= 1 && depth <= 3 {
                            register_external_include_dir(src_dir);
                        }
                    }
                }
            }
        }

        // Add include directories discovered during analysis
        for include_dir in get_external_include_dirs() {
            if seen_include_dirs.insert(include_dir.clone()) {
                let flag = include_flag_for_dir_impl(&include_dir, msvc);
                args_vec.push(heap.alloc_str(&flag).to_value());
            }
        }

        // Add preprocessor defines from cc_compilation_context
        // MSVC uses /D, GCC/Clang uses -D
        let define_prefix = if msvc { "/D" } else { "-D" };
        if !cc_compilation_context.is_none() {
            for attr_name in &["defines", "local_defines"] {
                if let Ok(Some(defines_val)) = cc_compilation_context.get_attr(attr_name, heap) {
                    if !defines_val.is_none() {
                        let mut elements = Vec::new();
                        crate::interpreter::rule_defs::depset::collect_depset_elements(
                            defines_val,
                            &mut elements,
                            heap,
                        );
                        for elem in &elements {
                            let def = elem.to_str();
                            if !def.is_empty() {
                                args_vec.push(heap.alloc_str(&format!("{}{}", define_prefix, def)).to_value());
                            }
                        }
                    }
                }
            }
        }

        // Add dependency file generation flags if dotd_file is specified
        if !dotd_file.is_none() {
            if msvc {
                // MSVC: /showIncludes outputs deps to stdout (no .d file created).
                // Use actions.write() to create an empty .d file as a separate action,
                // since rules_cc declared the artifact and it must be bound.
                args_vec.push(heap.alloc_str("/showIncludes").to_value());
                if let Ok(Some(write_method)) = actions_value.get_attr("write", heap) {
                    let dotd_output = if let Ok(Some(m)) = dotd_file.get_attr("as_output", heap) {
                        eval.eval_function(m, &[], &[]).ok()
                    } else {
                        None
                    };
                    if let Some(dotd_out) = dotd_output {
                        // actions.write(output, content) - write empty string to .d file
                        let content = heap.alloc_str("").to_value();
                        let _ = eval.eval_function(write_method, &[dotd_out, content], &[]);
                    }
                }
            } else {
                // GCC/Clang: -MMD -MF <depfile>
                args_vec.push(heap.alloc_str("-MMD").to_value());
                args_vec.push(heap.alloc_str("-MF").to_value());
                if let Ok(Some(path_method)) = dotd_file.get_attr("as_output", heap) {
                    if let Ok(dotd_output) = eval.eval_function(path_method, &[], &[]) {
                        args_vec.push(dotd_output);
                    }
                }
            }
        }

        let arguments = heap.alloc(args_vec);

        // Build the outputs list with all output artifacts
        let mut outputs_vec: Vec<Value<'v>> = vec![output_artifact];

        // Helper to add auxiliary output artifact to the outputs list
        macro_rules! add_output {
            ($artifact:expr) => {
                if !$artifact.is_none() {
                    if let Ok(Some(method)) = $artifact.get_attr("as_output", heap) {
                        if let Ok(out) = eval.eval_function(method, &[], &[]) {
                            outputs_vec.push(out);
                        }
                    }
                }
            };
        }

        // Add auxiliary outputs if provided (dotd, diagnostics, gcno, dwo, lto)
        // On MSVC, dotd_file is handled by a separate write action
        if !msvc {
            add_output!(dotd_file);
        }
        add_output!(diagnostics_file);
        add_output!(gcno_file);
        add_output!(dwo_file);
        add_output!(lto_indexing_file);

        let outputs_list = heap.alloc(outputs_vec);

        // Build the progress message
        let progress_msg = heap
            .alloc_str(&format!("Compiling {}", source_path))
            .to_value();

        // Build named arguments for run()
        // run(arguments, outputs=outputs, mnemonic=mnemonic, progress_message=msg, identifier=id)
        // Use source path as identifier to disambiguate multiple compile actions
        let identifier = heap.alloc_str(&source_path).to_value();
        let named_args: Vec<(&str, Value<'v>)> = vec![
            ("outputs", outputs_list),
            ("mnemonic", heap.alloc_str(&action_name_str).to_value()),
            ("progress_message", progress_msg),
            ("identifier", identifier),
        ];

        // Invoke actions.run() using Starlark's function evaluation
        // This properly registers the action through Kuro's infrastructure
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(f, "  Calling run_method with:");
            let _ = writeln!(f, "    arguments: {}", arguments);
            let _ = writeln!(f, "    outputs: {}", outputs_list);
            let _ = writeln!(f, "    mnemonic: {}", action_name_str);
        }
        let run_result = eval.eval_function(run_method, &[arguments], &named_args);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "  run result: {:?}",
                run_result
                    .as_ref()
                    .map(|v| v.to_string())
                    .map_err(|e| e.to_string())
            );
        }

        Ok(NoneType)
    }

    /// Gets the artifact name for a given category.
    ///
    /// Categories include: "object_file", "pic_object_file", "executable", etc.
    #[allow(unused_variables)]
    fn get_artifact_name_for_category<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] category: &str,
        #[starlark(require = named, default = "")] output_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[get_artifact_name_for_category] category={:?}, output_name={:?}",
                category, output_name
            );
        }
        // TODO(cc_common): Implement proper artifact naming based on toolchain
        // For now, return basic naming conventions
        let name = if output_name.is_empty() {
            "output"
        } else {
            output_name
        };
        // Category names come in both uppercase (from rules_cc artifact_category_names struct)
        // and lowercase (from direct string usage). Normalize to uppercase for matching.
        // Platform-specific extensions: Windows uses .obj/.lib/.dll/.exe, Unix uses .o/.a/.so
        let windows = is_windows_host();
        let result = match category.to_uppercase().as_str() {
            // Object files
            "OBJECT_FILE" => if windows { format!("{}.obj", name) } else { format!("{}.o", name) },
            "PIC_OBJECT_FILE" => if windows { format!("{}.obj", name) } else { format!("{}.pic.o", name) },
            "PIC_FILE" => if windows { format!("{}.obj", name) } else { format!("{}.pic", name) },

            // Libraries
            "STATIC_LIBRARY" => if windows { format!("{}.lib", name) } else { format!("lib{}.a", name) },
            "ALWAYSLINK_STATIC_LIBRARY" => if windows { format!("{}.lo.lib", name) } else { format!("lib{}.lo", name) },
            "DYNAMIC_LIBRARY" => if windows { format!("{}.dll", name) } else { format!("lib{}.so", name) },
            "INTERFACE_LIBRARY" => if windows { format!("{}.if.lib", name) } else { format!("lib{}.so", name) },

            // Executables
            "EXECUTABLE" => if windows { format!("{}.exe", name) } else { name.to_owned() },

            // Dependency tracking
            "INCLUDED_FILE_LIST" => format!("{}.d", name),

            // Diagnostics
            "SERIALIZED_DIAGNOSTICS_FILE" => format!("{}.dia", name),

            // Headers
            "GENERATED_HEADER" => format!("{}.h", name),
            "PROCESSED_HEADER" => format!("{}.h", name),

            // C++20 modules
            "CPP_MODULE" => format!("{}.pcm", name),
            "CPP_MODULES_DDI" => format!("{}.ddi", name),
            "CPP_MODULES_INFO" => format!("{}.modinfo", name),
            "CPP_MODULES_MODMAP" => format!("{}.modmap", name),
            "CPP_MODULES_MODMAP_INPUT" => format!("{}.input_modmap", name),

            // Preprocessing
            "PREPROCESSED_C_SOURCE" => format!("{}.i", name),
            "PREPROCESSED_CPP_SOURCE" => format!("{}.ii", name),

            // Coverage (gcov)
            "COVERAGE_DATA_FILE" => format!("{}.gcno", name),
            "COVERAGE_NOTES_FILE" => format!("{}.gcda", name),

            // Other
            "CLIF_OUTPUT_PROTO" => format!("{}.opb", name),

            // Unknown category - use category as extension
            _ => format!("{}.{}", name, category),
        };
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(f, "  result = {:?}", result);
        }
        Ok(result)
    }

    /// Combines toolchain variables from multiple sources.
    ///
    /// Takes 2 or 3 positional arguments - base variables plus 1-2 override variables.
    /// Variables are merged, with later arguments taking precedence.
    #[allow(unused_variables)]
    fn combine_cc_toolchain_variables<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] base: Value<'v>,
        #[starlark(require = pos)] first_override: Value<'v>,
        #[starlark(default = NoneType)] second_override: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let mut merged: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();

        // Merge all variable dicts: base + first_override + second_override (later values override)
        for vars_val in [base, first_override, second_override] {
            if vars_val.is_none() {
                continue;
            }
            // Try to downcast to CcToolchainVariables and iterate its inner dict
            if let Some(cv) = vars_val.downcast_ref::<CcToolchainVariablesGen<Value<'v>>>() {
                let inner = cv.vars;
                if !inner.is_none() {
                    if let Some(dict_ref) = DictRef::from_value(inner) {
                        for (k, v) in dict_ref.iter() {
                            if let Ok(hashed) = k.get_hashed() {
                                merged.insert_hashed(hashed, v);
                            }
                        }
                    }
                }
            }
            // If it's not a CcToolchainVariables (e.g., empty depset from _build_variables), skip
        }

        let merged_dict = heap.alloc(Dict::new(merged));
        Ok(heap.alloc(CcToolchainVariablesGen { vars: merged_dict }))
    }

    /// Gets the rule context from an actions object.
    ///
    /// This is a workaround used by rules_cc to access ctx from actions.
    /// We preserve the real actions object so create_cc_compile_action can use it.
    #[allow(unused_variables)]
    fn actions2ctx_cheat<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Try to extract label info from the actions object
        let (cell_name, pkg_path, target_name) = (|| -> Option<(String, String, String)> {
            let analysis_actions = actions
                .downcast_ref::<crate::interpreter::rule_defs::context::AnalysisActions>(
            )?;
            let state = analysis_actions.state.try_borrow().ok()?;
            let registry = state.as_ref()?;
            let owner = registry.actions.owner();
            match owner {
                kuro_core::deferred::key::DeferredHolderKey::Base(
                    kuro_core::deferred::base_deferred_key::BaseDeferredKey::TargetLabel(label),
                ) => {
                    let cell = label.pkg().cell_name().as_str().to_owned();
                    let pkg = label.pkg().cell_relative_path().to_string();
                    let name = label.name().as_str().to_owned();
                    Some((cell, pkg, name))
                }
                _ => None,
            }
        })()
        .unwrap_or_else(|| ("".to_owned(), "stub".to_owned(), "stub".to_owned()));

        // Return a wrapper that preserves the real actions object and label info
        // This allows create_cc_compile_action to register real actions
        Ok(eval.heap().alloc(CtxCheatWithActions {
            actions,
            cell_name,
            pkg_path,
            target_name,
        }))
    }

    /// Creates CcToolchainVariables from a dictionary.
    #[allow(unused_variables)]
    fn cc_toolchain_variables<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] vars: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Wrap the variables dict in CcToolchainVariables
        Ok(eval.heap().alloc(CcToolchainVariablesGen { vars }))
    }

    /// Freezes a list to an immutable tuple.
    fn freeze<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        value: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Properly convert list to tuple for immutability
        // For now, just return the value as-is since this is a stub
        Ok(value)
    }

    /// Returns the execution requirements for a given action.
    ///
    /// Returns a list of execution requirements (like "requires-worker-protocol:json")
    /// that should be added to actions using the specified tool.
    #[allow(unused_variables)]
    fn get_tool_requirement_for_action<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty list - no special execution requirements
        Ok(eval.heap().alloc(Vec::<String>::new()))
    }

    /// Creates a tree artifact compile action template.
    fn create_cc_compile_action_template<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement tree artifact compile template
        Ok(NoneType)
    }

    /// Wraps link actions for platform compatibility.
    ///
    /// Arguments:
    /// - actions: The ctx.actions object
    /// - build_config: Build configuration (usually ctx.configuration), optional
    /// - use_shareable_artifact_factory: Whether to use shareable artifact factory, optional
    #[allow(unused_variables)]
    fn wrap_link_actions<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(default = NoneType)] build_config: Value<'v>,
        #[starlark(default = false)] use_shareable_artifact_factory: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement link action wrapping
        // Return a wrapper that proxies the actions object
        Ok(actions)
    }

    /// Gets the SONAME for a dynamic library.
    #[allow(unused_variables)]
    fn dynamic_library_soname<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] short_path: &str,
        #[starlark(require = pos)] preserve_name: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // Extract library name from the path for SONAME
        let basename = short_path.rsplit('/').next().unwrap_or(short_path);
        Ok(basename.to_owned())
    }

    /// Creates a symlink for a dynamic library.
    #[allow(unused_variables)]
    fn dynamic_library_symlink<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] actions: Value<'v>,
        #[starlark(require = pos)] artifact: Value<'v>,
        #[starlark(require = pos)] solib_dir: Value<'v>,
        #[starlark(require = pos)] preserve_name: bool,
        #[starlark(require = pos)] use_short_path: bool,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return the artifact unchanged - symlink creation is a stub
        Ok(artifact)
    }

    /// Interns a sequence for efficiency (returns it unchanged).
    #[allow(unused_variables)]
    fn intern_seq<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] value: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return the sequence unchanged - interning is just an optimization
        Ok(value)
    }

    /// Gets link arguments for a given feature configuration.
    ///
    /// This function extracts variables from build_variables and constructs
    /// the linker command line arguments. For rules_cc compatibility, this
    /// returns an Args-like list that can be passed to actions.run(arguments=...).
    ///
    /// The build_variables contain `libraries_to_link` which is a list of
    /// provider instances created by rules_cc:
    /// - _NamedLibraryInfo: type in {object_file, static_library, dynamic_library, interface_library}
    /// - _ObjectFileGroupInfo: type = object_file_group, has .object_files list
    /// - _VersionedLibraryInfo: type = versioned_dynamic_library, has .name and .path
    ///
    /// For dynamic_library type, .name is a short library name (e.g., "hello_lib")
    /// that should be emitted as -l<name>. For other types, .name is a full path.
    #[allow(unused_variables)]
    fn get_link_args<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: Value<'v>,
        #[starlark(require = named)] build_variables: Value<'v>,
        #[starlark(require = named, default = NoneType)] parameter_file_type: Value<'v>,
        // Kuro extension: Optional input artifacts for proper path resolution.
        #[starlark(require = named, default = NoneType)] input_artifacts: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Debug logging
        use std::io::Write;
        let mut debug_log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
            .ok();
        if let Some(ref mut f) = debug_log {
            let _ = writeln!(f, "[get_link_args] action_name={}", action_name);
        }

        // Get action name as string
        let action_name_str = action_name.unpack_str().unwrap_or("c++-link-executable");

        let mut args: Vec<Value<'v>> = Vec::new();

        // Helper to get a variable value from either CcToolchainVariables or a raw dict
        let get_var = |key: &str| -> Option<Value<'v>> {
            if let Some(v) = build_variables.get_attr(key, heap).ok().flatten() {
                return Some(v);
            }
            if let Some(dict_ref) = DictRef::from_value(build_variables) {
                if let Some(v) = dict_ref.get_str(key) {
                    return Some(v);
                }
            }
            None
        };

        // Build a map from artifact paths to artifact values (for resolving string paths)
        let mut artifact_map: std::collections::HashMap<String, Value<'v>> =
            std::collections::HashMap::new();
        if !input_artifacts.is_none() {
            let artifacts_iter =
                if let Ok(Some(to_list)) = input_artifacts.get_attr("to_list", heap) {
                    if let Ok(list_val) = eval.eval_function(to_list, &[], &[]) {
                        list_val.iterate(heap).ok()
                    } else {
                        None
                    }
                } else {
                    input_artifacts.iterate(heap).ok()
                };
            if let Some(iter) = artifacts_iter {
                for artifact in iter {
                    if let Ok(Some(short_path)) = artifact.get_attr("short_path", heap) {
                        if let Some(path_str) = short_path.unpack_str() {
                            artifact_map.insert(path_str.to_owned(), artifact);
                        }
                    }
                    if let Ok(Some(path_attr)) = artifact.get_attr("path", heap) {
                        if let Some(path_str) = path_attr.unpack_str() {
                            artifact_map.insert(path_str.to_owned(), artifact);
                        }
                    }
                }
            }
        }

        // --- Output path ---
        let msvc = is_windows_host();
        if let Some(output) = get_var("output_execpath") {
            if action_name_str.contains("static-library") {
                if msvc {
                    // MSVC lib.exe: /nologo /OUT:<path>
                    args.push(heap.alloc_str("/nologo").to_value());
                } else {
                    args.push(heap.alloc_str("rcs").to_value());
                }
            } else if action_name_str.contains("dynamic-library") {
                if msvc {
                    args.push(heap.alloc_str("/nologo").to_value());
                    args.push(heap.alloc_str("/DLL").to_value());
                } else {
                    args.push(heap.alloc_str("-shared").to_value());
                    args.push(heap.alloc_str("-o").to_value());
                }
            } else {
                if msvc {
                    args.push(heap.alloc_str("/nologo").to_value());
                } else {
                    args.push(heap.alloc_str("-o").to_value());
                }
            }

            // For MSVC, format output as /OUT:<path>
            let output_path_str = if let Some(s) = output.unpack_str() {
                Some(s.to_owned())
            } else if let Ok(Some(path)) = output.get_attr("path", heap) {
                path.unpack_str().map(|s| s.to_owned())
            } else {
                None
            };

            if msvc {
                if let Some(ref path) = output_path_str {
                    args.push(heap.alloc_str(&format!("/OUT:{}", path)).to_value());
                }
                // Also need to bind the artifact
                if let Ok(Some(as_output_method)) = output.get_attr("as_output", heap) {
                    let _ = eval.eval_function(as_output_method, &[], &[]);
                }
            } else if output.unpack_str().is_some() {
                args.push(output);
            } else {
                let path_result = output.get_attr("path", heap);
                if let Ok(Some(as_output_method)) = output.get_attr("as_output", heap) {
                    match eval.eval_function(as_output_method, &[], &[]) {
                        Ok(output_artifact) => {
                            args.push(output_artifact);
                        }
                        Err(_) => {
                            if let Ok(Some(path)) = path_result {
                                args.push(path);
                            } else {
                                args.push(heap.alloc_str(&output.to_str()).to_value());
                            }
                        }
                    }
                } else if let Ok(Some(path)) = path_result {
                    args.push(path);
                } else {
                    args.push(heap.alloc_str(&output.to_str()).to_value());
                }
            }
        }

        // Helper: iterate a value that may be a list or depset
        // For depsets, call .to_list() first to get an iterable
        let iterate_value =
            |val: Value<'v>, eval_ref: &mut Evaluator<'v, '_, '_>| -> Vec<Value<'v>> {
                let h = eval_ref.heap();
                // Try to_list() for depsets
                if let Ok(Some(to_list_method)) = val.get_attr("to_list", h) {
                    if let Ok(list_val) = eval_ref.eval_function(to_list_method, &[], &[]) {
                        if let Ok(iter) = list_val.iterate(h) {
                            return iter.collect();
                        }
                    }
                }
                // Fall back to direct iteration (for lists)
                if let Ok(iter) = val.iterate(h) {
                    iter.collect()
                } else {
                    Vec::new()
                }
            };

        // --- Library search directories ---
        let mut lib_search_dirs: Vec<String> = Vec::new();
        if let Some(dirs) = get_var("library_search_directories") {
            for dir in iterate_value(dirs, eval) {
                if let Some(dir_str) = dir.unpack_str() {
                    if !dir_str.is_empty() {
                        if msvc {
                            args.push(heap.alloc_str(&format!("/LIBPATH:{}", dir_str)).to_value());
                        } else {
                            args.push(heap.alloc_str(&format!("-L{}", dir_str)).to_value());
                        }
                        lib_search_dirs.push(dir_str.to_owned());
                    }
                }
            }
        }

        // Add MSVC system library paths
        if msvc {
            if let Some(tools) = get_msvc_tool_paths() {
                if !tools.msvc_lib.is_empty() {
                    args.push(heap.alloc_str(&format!("/LIBPATH:{}", tools.msvc_lib)).to_value());
                }
                if !tools.ucrt_lib.is_empty() {
                    args.push(heap.alloc_str(&format!("/LIBPATH:{}", tools.ucrt_lib)).to_value());
                }
                if !tools.um_lib.is_empty() {
                    args.push(heap.alloc_str(&format!("/LIBPATH:{}", tools.um_lib)).to_value());
                }
            }
        }

        // --- Libraries to link ---
        // On Linux, wrap in --start-group/--end-group for circular dep resolution.
        // MSVC doesn't need this (it always resolves circular deps).
        let is_executable_link = action_name_str.contains("executable");
        if is_executable_link && !msvc {
            args.push(heap.alloc_str("-Wl,--start-group").to_value());
        }
        // Process based on .type field from rules_cc provider instances
        if let Some(libs) = get_var("libraries_to_link") {
            if let Ok(iter) = libs.iterate(heap) {
                for lib in iter {
                    // Get the library type to determine how to format the argument
                    let lib_type = lib
                        .get_attr("type", heap)
                        .ok()
                        .flatten()
                        .and_then(|v| v.unpack_str().map(|s| s.to_owned()));

                    let is_whole_archive = lib
                        .get_attr("is_whole_archive", heap)
                        .ok()
                        .flatten()
                        .map(|v| v.unpack_bool() == Some(true))
                        .unwrap_or(false);

                    if is_whole_archive {
                        args.push(heap.alloc_str("-Wl,--whole-archive").to_value());
                    }

                    match lib_type.as_deref() {
                        Some("dynamic_library") => {
                            // Dynamic library: emit -l<name> flag
                            // .name is a short name like "hello_lib" (from "libhello_lib.so")
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    args.push(
                                        heap.alloc_str(&format!("-l{}", name_str)).to_value(),
                                    );
                                }
                            }
                        }
                        Some("versioned_dynamic_library") => {
                            // Versioned dynamic library: use -l:<name> for exact match
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    args.push(
                                        heap.alloc_str(&format!("-l:{}", name_str)).to_value(),
                                    );
                                }
                            }
                        }
                        Some("object_file_group") => {
                            // Object file group: iterate .object_files and add each
                            if let Some(object_files) =
                                lib.get_attr("object_files", heap).ok().flatten()
                            {
                                if let Ok(obj_iter) = object_files.iterate(heap) {
                                    for obj in obj_iter {
                                        if obj.get_type() == "File" {
                                            args.push(obj);
                                        } else if let Some(path_str) = obj.unpack_str() {
                                            push_path_or_artifact(
                                                path_str,
                                                &artifact_map,
                                                &mut args,
                                                heap,
                                            );
                                        } else {
                                            args.push(obj);
                                        }
                                    }
                                }
                            }
                        }
                        Some("object_file")
                        | Some("static_library")
                        | Some("interface_library") => {
                            // These types use .name as a full path
                            if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    push_path_or_artifact(name_str, &artifact_map, &mut args, heap);
                                } else {
                                    args.push(name);
                                }
                            }
                        }
                        _ => {
                            // Unknown type or no type field - use legacy fallback
                            if let Some(path_str) = lib.unpack_str() {
                                push_path_or_artifact(path_str, &artifact_map, &mut args, heap);
                            } else if let Some(artifact) =
                                lib.get_attr("artifact", heap).ok().flatten()
                            {
                                if artifact.is_none() {
                                    if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                        if let Some(name_str) = name.unpack_str() {
                                            push_path_or_artifact(
                                                name_str,
                                                &artifact_map,
                                                &mut args,
                                                heap,
                                            );
                                        } else {
                                            args.push(name);
                                        }
                                    }
                                } else {
                                    args.push(artifact);
                                }
                            } else if let Some(name) = lib.get_attr("name", heap).ok().flatten() {
                                if let Some(name_str) = name.unpack_str() {
                                    push_path_or_artifact(name_str, &artifact_map, &mut args, heap);
                                } else {
                                    args.push(name);
                                }
                            } else if lib.get_type() == "File" {
                                args.push(lib);
                            } else {
                                let path_str = lib.to_str();
                                push_path_or_artifact(&path_str, &artifact_map, &mut args, heap);
                            }
                        }
                    }

                    if is_whole_archive {
                        args.push(heap.alloc_str("-Wl,--no-whole-archive").to_value());
                    }
                }
            }
        }

        if is_executable_link && !msvc {
            args.push(heap.alloc_str("-Wl,--end-group").to_value());
        }

        // --- User link flags ---
        // Deduplicate flags while preserving order, since transitive depsets
        // can produce massive duplication (e.g., -lm -lpthread repeated 2000+ times).
        if let Some(flags) = get_var("user_link_flags") {
            let mut seen_flags = std::collections::HashSet::new();
            if let Ok(iter) = flags.iterate(heap) {
                for flag in iter {
                    if let Some(s) = flag.unpack_str() {
                        if seen_flags.insert(s.to_owned()) {
                            args.push(flag);
                        }
                    }
                }
            }
        }

        // --- Runtime library search directories (-rpath flags) ---
        // Use $ORIGIN-relative paths so the runtime linker can find shared libraries
        // regardless of the working directory when the binary is executed.
        let output_dir: Option<String> = get_var("output_execpath").and_then(|v| {
            let path_str = if let Some(s) = v.unpack_str() {
                s.to_owned()
            } else if let Ok(Some(path_attr)) = v.get_attr("path", heap) {
                path_attr
                    .unpack_str()
                    .map(|s| s.to_owned())
                    .unwrap_or_else(|| v.to_str())
            } else {
                v.to_str()
            };
            std::path::Path::new(&path_str)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
        });

        let make_origin_rpath = |dir_str: &str| -> String {
            // runtime_library_search_directories paths are relative to the binary's
            // output directory (i.e., relative to $ORIGIN). Use them directly.
            // e.g. dir_str="../__hello_lib__" → "-Wl,-rpath,$ORIGIN/../__hello_lib__"
            format!("-Wl,-rpath,$ORIGIN/{}", dir_str)
        };

        let mut has_rpath = false;
        let mut seen_rpaths: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Some(dirs) = get_var("runtime_library_search_directories") {
            for dir in iterate_value(dirs, eval) {
                if let Some(dir_str) = dir.unpack_str() {
                    if !dir_str.is_empty() {
                        let rpath = make_origin_rpath(dir_str);
                        if seen_rpaths.insert(rpath.clone()) {
                            args.push(heap.alloc_str(&rpath).to_value());
                        }
                        has_rpath = true;
                    }
                }
            }
        }
        // Fallback: use library_search_directories for rpath if no explicit rpath dirs
        if !has_rpath && !lib_search_dirs.is_empty() {
            for dir_str in &lib_search_dirs {
                let rpath = make_origin_rpath(dir_str);
                if seen_rpaths.insert(rpath.clone()) {
                    args.push(heap.alloc_str(&rpath).to_value());
                }
            }
        }

        Ok(heap.alloc(args))
    }

    /// Declares a compile output file.
    ///
    /// This function uses the real AnalysisActions from the ctx parameter
    /// to create a properly registered output artifact.
    fn declare_compile_output_file<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] label: Value<'v>,
        #[starlark(require = named, default = "")] output_name: &str,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[declare_compile_output_file] output_name={:?}",
                output_name
            );
        }

        let heap = eval.heap();
        let _ = (label, configuration); // Unused for now

        // Get the real actions from ctx.actions
        let actions_value = match ctx.get_attr("actions", heap) {
            Ok(Some(actions)) => actions,
            _ => {
                // Fallback to stub if no real actions available
                return Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.to_owned(),
                }));
            }
        };

        // Try to get the declare_file method
        let declare_file_method = match actions_value.get_attr("declare_file", heap) {
            Ok(Some(method)) => method,
            _ => {
                // Fallback to stub if declare_file not available
                return Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.to_owned(),
                }));
            }
        };

        // Call declare_file(output_name) using Starlark's function evaluation
        let filename = heap.alloc_str(output_name).to_value();
        match eval.eval_function(declare_file_method, &[filename], &[]) {
            Ok(artifact) => {
                // Log the artifact's path attribute
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(f, "  declared artifact: {}", artifact);
                    if let Ok(Some(path)) = artifact.get_attr("path", heap) {
                        let _ = writeln!(f, "  artifact.path = {}", path);
                    }
                }
                Ok(artifact)
            }
            Err(e) => {
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/cc_common_compile.log")
                {
                    let _ = writeln!(f, "  declare_file error: {}", e);
                }
                // Fallback to stub on error
                Ok(heap.alloc(CtxCheatArtifactStub {
                    path: output_name.to_owned(),
                }))
            }
        }
    }

    /// Declares an auxiliary output file (dwo, gcno, etc.).
    #[allow(unused_variables)]
    fn declare_other_output_file<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] source_file: Value<'v>,
        #[starlark(require = named, default = "")] extension: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement other output declaration
        Ok(NoneType)
    }

    /// Checks if an artifact is a tree artifact.
    fn is_tree_artifact<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        _artifact: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // TODO(cc_common): Check actual artifact type
        Ok(false)
    }

    /// Computes the output name prefix directory.
    ///
    /// This returns the directory prefix for object files, typically `_objs/{purpose}`.
    /// In Bazel, this creates object files in a target-specific subdirectory.
    #[allow(unused_variables)]
    fn compute_output_name_prefix_dir<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] purpose: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(f, "[compute_output_name_prefix_dir] purpose={}", purpose);
        }

        // The purpose is typically the target name or a unique identifier.
        // Object files should go in `_objs/{purpose}/` directory.
        if purpose.is_none() {
            // No purpose specified, use a default
            return Ok("_objs".to_owned());
        }

        // Try to get a string value from purpose
        if let Some(purpose_str) = purpose.unpack_str() {
            // If purpose is empty string, return just "_objs" without trailing slash
            // to avoid double slashes like "_objs//main.o"
            if purpose_str.is_empty() {
                return Ok("_objs".to_owned());
            }
            return Ok(format!("_objs/{}", purpose_str));
        }

        // If purpose has a 'name' attribute (like a Label), use that
        if let Ok(Some(name)) = purpose.get_attr("name", eval.heap()) {
            if let Some(name_str) = name.unpack_str() {
                if name_str.is_empty() {
                    return Ok("_objs".to_owned());
                }
                return Ok(format!("_objs/{}", name_str));
            }
        }

        // Fallback: just use _objs
        Ok("_objs".to_owned())
    }

    /// Interns a string sequence variable value for efficiency.
    fn intern_string_sequence_variable_value<'v>(
        #[starlark(this)] _this: &CcCommonInternal,
        value: Value<'v>,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // For now, just return the value as-is
        // TODO(cc_common): Implement proper interning
        Ok(value)
    }

    /// Gets per-file compile options.
    #[allow(unused_variables)]
    fn per_file_copts<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = pos)] cpp_configuration: Value<'v>,
        #[starlark(require = pos)] source_file: Value<'v>,
        #[starlark(require = pos)] label: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement per-file copts
        Ok(eval.heap().alloc(AllocList::EMPTY))
    }

    /// Checks access to private API (allowlist enforcement).
    #[allow(unused_variables)]
    fn check_private_api<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named)] allowlist: Value<'v>,
        #[starlark(require = named, default = 1)] depth: i32,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // Always allow for now
        Ok(true)
    }

    /// Creates a HeaderInfo struct.
    #[allow(unused_variables)]
    fn create_header_info<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] header_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_header_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_public_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_private_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_pic_module: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return a HeaderInfo stub with the necessary attributes
        Ok(eval.heap().alloc(HeaderInfoStub))
    }

    /// Creates a HeaderInfo struct with dependency tracking.
    #[allow(unused_variables)]
    fn create_header_info_with_deps<'v>(
        #[starlark(this)] this: &CcCommonInternal,
        #[starlark(require = named, default = NoneType)] headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] modular_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] deps: Value<'v>,
        #[starlark(require = named, default = NoneType)] header_info: Value<'v>,
        #[starlark(require = named, default = NoneType)] merged_deps: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper HeaderInfo with deps
        Ok(eval.heap().alloc(HeaderInfoStub))
    }
}

// ============================================================================
// CcCommonModule - The main cc_common module
// ============================================================================

/// The cc_common module provides C/C++ compilation support.
///
/// This is Bazel's native module for C++ build configuration. For Bazel 9.0+,
/// most of the actual compilation logic is in pure Starlark (rules_cc), but
/// the native cc_common module provides low-level primitives.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcCommonModule;

impl Display for CcCommonModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cc_common")
    }
}

starlark_simple_value!(CcCommonModule);

#[starlark_value(type = "cc_common")]
impl<'v> StarlarkValue<'v> for CcCommonModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_common_module_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        // Report which attributes exist for hasattr() checks
        matches!(
            attribute,
            "internal_DO_NOT_USE"
                | "get_tool_for_action"
                | "get_execution_requirements"
                | "action_is_enabled"
                | "get_memory_inefficient_command_line"
                | "get_environment_variables"
                | "empty_variables"
                | "do_not_use_tools_cpp_compiler_present"
                | "CcToolchainInfo"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "do_not_use_tools_cpp_compiler_present" => Some(Value::new_bool(true)),
            "CcToolchainInfo" => Some(heap.alloc(CcToolchainInfoProvider)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "internal_DO_NOT_USE".to_owned(),
            "get_tool_for_action".to_owned(),
            "get_execution_requirements".to_owned(),
            "action_is_enabled".to_owned(),
            "get_memory_inefficient_command_line".to_owned(),
            "get_environment_variables".to_owned(),
            "empty_variables".to_owned(),
            "do_not_use_tools_cpp_compiler_present".to_owned(),
            "CcToolchainInfo".to_owned(),
        ]
    }
}

/// Methods on the cc_common module.
#[starlark_module]
fn cc_common_module_methods(builder: &mut MethodsBuilder) {
    /// Returns the internal cc_common API struct.
    ///
    /// Used by rules_cc via: cc_internal = cc_common.internal_DO_NOT_USE()
    #[starlark(attribute)]
    fn internal_DO_NOT_USE(this: &CcCommonModule) -> starlark::Result<CcCommonInternal> {
        let _ = this;
        Ok(CcCommonInternal)
    }

    /// Configures C++ features based on toolchain and requested features.
    ///
    /// Returns a FeatureConfiguration that controls which compiler flags are enabled.
    #[allow(unused_variables)]
    fn configure_features<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] requested_features: Value<'v>,
        #[starlark(require = named, default = NoneType)] unsupported_features: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<FeatureConfiguration> {
        let _ = (this, ctx, cc_toolchain);
        let heap = eval.heap();

        // Collect requested features from the list
        let mut req: Vec<String> = Vec::new();
        if !requested_features.is_none() {
            if let Ok(iter) = requested_features.iterate(heap) {
                for item in iter {
                    if let Some(s) = item.unpack_str() {
                        req.push(s.to_owned());
                    }
                }
            }
        }

        // Collect unsupported features from the list
        let mut unsup: Vec<String> = Vec::new();
        if !unsupported_features.is_none() {
            if let Ok(iter) = unsupported_features.iterate(heap) {
                for item in iter {
                    if let Some(s) = item.unpack_str() {
                        unsup.push(s.to_owned());
                    }
                }
            }
        }

        Ok(FeatureConfiguration::new(req, unsup))
    }

    /// Compiles C/C++ source files.
    ///
    /// This is the main compilation function that creates compile actions for each
    /// source file and returns compilation context and outputs.
    ///
    /// Returns a tuple of (CcCompilationContext, CompilationOutputs).
    #[allow(unused_variables)]
    fn compile<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] srcs: Value<'v>,
        #[starlark(require = named, default = NoneType)] public_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] private_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] textual_hdrs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] loose_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] quote_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] system_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] framework_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] local_defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] include_prefix: Value<'v>,
        #[starlark(require = named, default = NoneType)] strip_include_prefix: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_compile_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] conly_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] cxx_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] compilation_contexts: Value<'v>,
        #[starlark(require = named, default = NoneType)] implementation_compilation_contexts: Value<
            'v,
        >,
        #[starlark(require = named, default = false)] disallow_pic_outputs: bool,
        #[starlark(require = named, default = false)] disallow_nopic_outputs: bool,
        #[starlark(require = named, default = NoneType)] additional_include_scanning_roots: Value<
            'v,
        >,
        #[starlark(require = named, default = false)] do_not_generate_module_map: bool,
        #[starlark(require = named, default = false)] code_coverage_enabled: bool,
        #[starlark(require = named, default = NoneType)] hdrs_checking_mode: Value<'v>,
        #[starlark(require = named, default = NoneType)] variables_extension: Value<'v>,
        #[starlark(require = named, default = NoneType)] language: Value<'v>,
        #[starlark(require = named, default = NoneType)] purpose: Value<'v>,
        #[starlark(require = named, default = NoneType)] copts_filter: Value<'v>,
        #[starlark(require = named, default = NoneType)] separate_module_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] module_interfaces: Value<'v>,
        #[starlark(require = named, default = NoneType)] non_compilation_additional_inputs: Value<
            'v,
        >,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Write debug to file
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/cc_common_compile.log")
        {
            let _ = writeln!(
                f,
                "[cc_common.compile] name={}, srcs={}, srcs.is_none()={}",
                name,
                srcs,
                srcs.is_none()
            );
        }

        // Collect source files to compile
        let mut object_files: Vec<Value<'v>> = Vec::new();
        let mut pic_object_files: Vec<Value<'v>> = Vec::new();

        // Get the declare_file method from actions
        let declare_file_method = actions.get_attr("declare_file", heap).ok().flatten();
        let run_method = actions.get_attr("run", heap).ok().flatten();

        // Register include directories from compilation_contexts (deps' contexts)
        if !compilation_contexts.is_none() {
            if let Ok(iter) = compilation_contexts.iterate(heap) {
                for ctx in iter {
                    // Extract includes from each dep compilation context
                    for attr_name in &["includes", "system_includes", "quote_includes"] {
                        if let Ok(Some(includes_val)) = ctx.get_attr(attr_name, heap) {
                            if !includes_val.is_none() {
                                let mut elements = Vec::new();
                                crate::interpreter::rule_defs::depset::collect_depset_elements(
                                    includes_val,
                                    &mut elements,
                                    heap,
                                );
                                for elem in &elements {
                                    let dir = elem.to_str();
                                    if !dir.is_empty() {
                                        register_external_include_dir(&dir);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Handle strip_include_prefix - register the resolved directory as an include path
        if let Some(strip_prefix) = strip_include_prefix.unpack_str() {
            if !strip_prefix.is_empty() {
                // strip_include_prefix is relative to the repo root, e.g. "/third_party/utf8_range"
                // We need to determine the repo name from the source paths
                if !srcs.is_none() {
                    if let Ok(iter) = srcs.iterate(heap) {
                        for src_tuple in iter {
                            let src = src_tuple
                                .at(heap.alloc(0i32).to_value(), heap)
                                .unwrap_or(src_tuple);
                            if let Some(src_path_raw) = src
                                .get_attr("path", heap)
                                .ok()
                                .flatten()
                                .and_then(|v| v.unpack_str())
                            {
                                // Normalize buck-out/v2/external_cells/... to external/...
                                let normalized;
                                let src_path: &str =
                                    if let Some(n) = normalize_external_cells_path(src_path_raw) {
                                        normalized = n;
                                        &normalized
                                    } else {
                                        src_path_raw
                                    };
                                if src_path.starts_with("external/") {
                                    if let Some(second_slash) = src_path[9..].find('/') {
                                        let repo = &src_path[..9 + second_slash];
                                        let prefix = strip_prefix.trim_start_matches('/');
                                        let include_dir = format!("{}/{}", repo, prefix);
                                        register_external_include_dir(&include_dir);
                                    }
                                }
                                break; // Only need one source to determine repo
                            }
                        }
                    }
                }
            }
        }

        // Process source files if provided
        // srcs is a list of (Artifact, Label) tuples from cc_helper.get_srcs()
        if !srcs.is_none() {
            // Try to iterate over srcs
            if let Ok(iter) = srcs.iterate(heap) {
                let items: Vec<_> = iter.collect();
                for src_tuple in items {
                    // Extract the artifact from the (Artifact, Label) tuple
                    // Try tuple index first, then fall back to treating it as artifact directly
                    let src = src_tuple
                        .at(heap.alloc(0i32).to_value(), heap)
                        .unwrap_or(src_tuple);

                    // Get source file path
                    let src_path = src
                        .get_attr("path", heap)
                        .ok()
                        .flatten()
                        .and_then(|v| v.unpack_str())
                        .unwrap_or("unknown.c");

                    // Register include dirs derived from source path for cross-target use
                    if src_path.starts_with("external/") {
                        if let Some(second_slash) = src_path[9..].find('/') {
                            let repo_dir = &src_path[..9 + second_slash];
                            register_external_include_dir(repo_dir);
                            // Also register <repo>/src/ if the source is under src/
                            if let Some(src_idx) = src_path.find("/src/") {
                                register_external_include_dir(&src_path[..src_idx + 5]);
                            }
                        }
                    }

                    // Determine output filename (replace extension with .o)
                    let basename = src_path.rsplit('/').next().unwrap_or(src_path);
                    let output_name = if let Some(dot_pos) = basename.rfind('.') {
                        format!("_objs/{}/{}.o", name, &basename[..dot_pos])
                    } else {
                        format!("_objs/{}/{}.o", name, basename)
                    };
                    let pic_output_name = if let Some(dot_pos) = basename.rfind('.') {
                        format!("_objs/{}/{}.pic.o", name, &basename[..dot_pos])
                    } else {
                        format!("_objs/{}/{}.pic.o", name, basename)
                    };

                    // Log what we're about to do
                    if let Ok(mut f) = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("/tmp/cc_common_compile.log")
                    {
                        let _ = writeln!(
                            f,
                            "  Declaring output: {}, pic: {}",
                            output_name, pic_output_name
                        );
                    }

                    // Declare output files
                    if let Some(declare_file) = declare_file_method {
                        // Regular object file
                        let output_file = eval.eval_function(
                            declare_file,
                            &[heap.alloc_str(&output_name).to_value()],
                            &[],
                        );
                        if let Ok(mut f) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open("/tmp/cc_common_compile.log")
                        {
                            let _ = writeln!(
                                f,
                                "  declare_file result: {:?}",
                                output_file.as_ref().map(|v| v.to_string())
                            );
                        }
                        let output_file = output_file.ok();

                        // PIC object file
                        let pic_output_file = eval
                            .eval_function(
                                declare_file,
                                &[heap.alloc_str(&pic_output_name).to_value()],
                                &[],
                            )
                            .ok();

                        // Register compile action if run method available
                        if let (Some(run), Some(out), Some(pic_out)) =
                            (run_method, output_file, pic_output_file)
                        {
                            // Get output as output artifact
                            let output_artifact = out
                                .get_attr("as_output", heap)
                                .ok()
                                .flatten()
                                .and_then(|method| eval.eval_function(method, &[], &[]).ok())
                                .unwrap_or(out);
                            let pic_output_artifact = pic_out
                                .get_attr("as_output", heap)
                                .ok()
                                .flatten()
                                .and_then(|method| eval.eval_function(method, &[], &[]).ok())
                                .unwrap_or(pic_out);

                            // Build compile command: <compiler> -c src -o output
                            let host_compiler = match std::env::consts::OS {
                                "windows" => "cl.exe",
                                "macos" => "/usr/bin/clang",
                                _ => "/usr/bin/gcc",
                            };
                            let args = heap.alloc(vec![
                                heap.alloc_str(host_compiler).to_value(),
                                heap.alloc_str("-c").to_value(),
                                src,
                                heap.alloc_str("-o").to_value(),
                                output_artifact,
                            ]);
                            let outputs_list = heap.alloc(vec![output_artifact]);
                            let progress = heap
                                .alloc_str(&format!("Compiling {}", basename))
                                .to_value();

                            // Call actions.run() for regular compile
                            // Use unique identifier to avoid "multiple actions with same category" error
                            let identifier = heap.alloc_str(&format!("{}.o", basename)).to_value();
                            let run_result = eval.eval_function(
                                run,
                                &[args],
                                &[
                                    ("outputs", outputs_list),
                                    ("category", heap.alloc_str("cpp_compile").to_value()),
                                    ("identifier", identifier),
                                    ("progress_message", progress),
                                ],
                            );
                            if let Ok(mut f) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open("/tmp/cc_common_compile.log")
                            {
                                let _ = writeln!(
                                    f,
                                    "  actions.run result (regular): {:?}",
                                    run_result
                                        .as_ref()
                                        .map(|v| v.to_string())
                                        .map_err(|e| e.to_string())
                                );
                            }

                            // Register PIC compile action with unique identifier
                            let pic_args = heap.alloc(vec![
                                heap.alloc_str(host_compiler).to_value(),
                                heap.alloc_str("-c").to_value(),
                                heap.alloc_str("-fPIC").to_value(),
                                src,
                                heap.alloc_str("-o").to_value(),
                                pic_output_artifact,
                            ]);
                            let pic_outputs_list = heap.alloc(vec![pic_output_artifact]);
                            let pic_progress = heap
                                .alloc_str(&format!("Compiling {} (PIC)", basename))
                                .to_value();
                            let pic_identifier =
                                heap.alloc_str(&format!("{}.pic.o", basename)).to_value();

                            let _ = eval.eval_function(
                                run,
                                &[pic_args],
                                &[
                                    ("outputs", pic_outputs_list),
                                    ("category", heap.alloc_str("cpp_compile").to_value()),
                                    ("identifier", pic_identifier),
                                    ("progress_message", pic_progress),
                                ],
                            );

                            object_files.push(out);
                            pic_object_files.push(pic_out);
                        }
                    }
                }
            }
        }

        // Create compilation context
        let none_val = Value::new_none();
        let compilation_context = heap.alloc(CcCompilationContextGen {
            headers: none_val,
            includes: none_val,
            quote_includes: none_val,
            system_includes: none_val,
            framework_includes: none_val,
            defines: none_val,
            local_defines: none_val,
        });

        // Create compilation outputs
        // Return lists of object files - these support len() which is needed by rules_cc
        let objects_list = heap.alloc(object_files.clone());
        let pic_objects_list = heap.alloc(pic_object_files.clone());
        let compilation_outputs = heap.alloc(CompilationOutputsGen {
            objects: objects_list,
            pic_objects: pic_objects_list,
        });

        // Return tuple of (compilation_context, compilation_outputs)
        Ok(heap.alloc((compilation_context, compilation_outputs)))
    }

    /// Links C++ code into a binary or shared library.
    ///
    /// This is the core linking function that rules_cc calls to create
    /// executables and shared libraries from compilation outputs.
    #[allow(unused_variables)]
    fn link<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = "c++")] language: &str,
        #[starlark(require = named, default = "executable")] output_type: &str,
        #[starlark(require = named, default = true)] link_deps_statically: bool,
        #[starlark(require = named, default = NoneType)] compilation_outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] linking_contexts: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_link_flags: Value<'v>,
        #[starlark(require = named, default = 0)] stamp: i32,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] variables_extension: Value<'v>,
        #[starlark(require = named, default = NoneType)] grep_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] main_output: Value<'v>,
        #[starlark(require = named, default = NoneType)] use_test_only_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] pdb_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] win_def_file: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();

        // Get the declare_file and run methods from actions
        let declare_file_method = actions.get_attr("declare_file", heap).ok().flatten();
        let run_method = actions.get_attr("run", heap).ok().flatten();

        // Determine action name based on output type
        let action_name = match output_type {
            "dynamic_library" => "c++-link-dynamic-library",
            "static_library" => "c++-link-static-library",
            _ => "c++-link-executable",
        };

        // Determine output extension based on output type and platform
        let is_dynamic = output_type == "dynamic_library";
        let is_static = output_type == "static_library";
        let output_ext = if is_static {
            if is_windows_host() { ".lib" } else { ".a" }
        } else if is_dynamic {
            if is_windows_host() { ".dll" } else if std::env::consts::OS == "macos" { ".dylib" } else { ".so" }
        } else {
            if is_windows_host() { ".exe" } else { "" }
        };

        let output_name = format!("{}{}", name, output_ext);

        // Declare output file
        let output_file = if let Some(declare_file) = declare_file_method {
            eval.eval_function(
                declare_file,
                &[heap.alloc_str(&output_name).to_value()],
                &[],
            )
            .ok()
        } else {
            None
        };

        if let (Some(run), Some(out)) = (run_method, output_file) {
            let output_artifact = out
                .get_attr("as_output", heap)
                .ok()
                .flatten()
                .and_then(|method| eval.eval_function(method, &[], &[]).ok())
                .unwrap_or(out);

            // Get linker tool path
            let linker_tool = match std::env::consts::OS {
                "windows" => {
                    let msvc = get_msvc_tool_paths();
                    if is_static {
                        msvc.as_ref().map(|t| t.lib.clone()).unwrap_or_else(|| "lib.exe".to_owned())
                    } else {
                        msvc.as_ref().map(|t| t.link.clone()).unwrap_or_else(|| "link.exe".to_owned())
                    }
                }
                "macos" => {
                    if is_static { "/usr/bin/ar".to_owned() } else { "/usr/bin/clang++".to_owned() }
                }
                _ => {
                    if is_static { "/usr/bin/ar".to_owned() } else { "/usr/bin/g++".to_owned() }
                }
            };

            // Build link command arguments
            let mut args: Vec<Value<'v>> = Vec::new();
            args.push(heap.alloc_str(&linker_tool).to_value());

            if is_static {
                // Static library: ar rcs output.a obj1.o obj2.o ...
                if !is_windows_host() {
                    args.push(heap.alloc_str("rcs").to_value());
                }
                args.push(output_artifact);
                if is_windows_host() {
                    // MSVC lib.exe: /OUT:output.lib obj1.obj obj2.obj
                    // Replace the last push with /OUT: flag
                    args.pop();
                    let out_flag = format!("/OUT:{}", output_artifact);
                    args.push(heap.alloc_str(&out_flag).to_value());
                }
            } else {
                // Executable or shared library
                if is_windows_host() {
                    args.push(heap.alloc_str(&format!("/OUT:{}", output_artifact)).to_value());
                    if is_dynamic {
                        args.push(heap.alloc_str("/DLL").to_value());
                    }
                } else {
                    args.push(heap.alloc_str("-o").to_value());
                    args.push(output_artifact);
                    if is_dynamic {
                        args.push(heap.alloc_str("-shared").to_value());
                    }
                }
            }

            // Collect object files from compilation_outputs
            if !compilation_outputs.is_none() {
                // Try objects attribute first (regular objects)
                if let Ok(Some(objects)) = compilation_outputs.get_attr("objects", heap) {
                    if !objects.is_none() {
                        if let Ok(iter) = objects.iterate(heap) {
                            for obj in iter {
                                args.push(obj);
                            }
                        }
                    }
                }
                // Also try pic_objects if no regular objects
                if let Ok(Some(pic_objects)) = compilation_outputs.get_attr("pic_objects", heap) {
                    if !pic_objects.is_none() {
                        if let Ok(iter) = pic_objects.iterate(heap) {
                            for obj in iter {
                                args.push(obj);
                            }
                        }
                    }
                }
            }

            // Collect linker inputs from linking_contexts
            if !linking_contexts.is_none() {
                if let Ok(iter) = linking_contexts.iterate(heap) {
                    for ctx_val in iter {
                        // Each linking_context has linker_inputs (a depset)
                        if let Ok(Some(linker_inputs)) = ctx_val.get_attr("linker_inputs", heap) {
                            if !linker_inputs.is_none() {
                                // Try to iterate through linker inputs (depset)
                                let mut elements = Vec::new();
                                crate::interpreter::rule_defs::depset::collect_depset_elements(
                                    linker_inputs,
                                    &mut elements,
                                    heap,
                                );
                                for input in elements {
                                    // Each linker input may have libraries
                                    if let Ok(Some(libraries)) = input.get_attr("libraries", heap) {
                                        if !libraries.is_none() {
                                            if let Ok(lib_iter) = libraries.iterate(heap) {
                                                for lib in lib_iter {
                                                    // Library_to_link has static_library, dynamic_library, etc.
                                                    if let Ok(Some(static_lib)) =
                                                        lib.get_attr("static_library", heap)
                                                    {
                                                        if !static_lib.is_none() {
                                                            args.push(static_lib);
                                                        }
                                                    }
                                                    if let Ok(Some(dynamic_lib)) =
                                                        lib.get_attr("dynamic_library", heap)
                                                    {
                                                        if !dynamic_lib.is_none() {
                                                            args.push(dynamic_lib);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Add user link flags
            if !user_link_flags.is_none() {
                if let Ok(iter) = user_link_flags.iterate(heap) {
                    for flag in iter {
                        args.push(flag);
                    }
                }
            }

            let args_val = heap.alloc(args);
            let outputs_list = heap.alloc(vec![output_artifact]);
            let progress = heap
                .alloc_str(&format!("Linking {}", output_name))
                .to_value();
            let category = if is_static {
                "cpp_link_static_library"
            } else if is_dynamic {
                "cpp_link_dynamic_library"
            } else {
                "cpp_link_executable"
            };

            let _ = eval.eval_function(
                run,
                &[args_val],
                &[
                    ("outputs", outputs_list),
                    ("category", heap.alloc_str(category).to_value()),
                    ("identifier", heap.alloc_str(&output_name).to_value()),
                    ("progress_message", progress),
                ],
            );

            // Create library_to_link if output is a library
            let library_to_link = if is_static || is_dynamic {
                heap.alloc(LibraryToLinkGen {
                    static_library: if is_static { out } else { Value::new_none() },
                    pic_static_library: Value::new_none(),
                    dynamic_library: if is_dynamic { out } else { Value::new_none() },
                    interface_library: Value::new_none(),
                    objects: Value::new_none(),
                    pic_objects: Value::new_none(),
                    alwayslink: false,
                })
            } else {
                Value::new_none()
            };

            // Return CcLinkingOutputs
            let executable = if !is_static && !is_dynamic { out } else { Value::new_none() };
            let linking_outputs = heap.alloc(CcLinkingOutputsGen { library_to_link, executable });

            Ok(linking_outputs)
        } else {
            // Fallback: return empty linking outputs
            Ok(heap.alloc(CcLinkingOutputsGen {
                library_to_link: Value::new_none(),
                executable: Value::new_none(),
            }))
        }
    }

    /// Gets the tool path for a given action.
    #[allow(unused_variables)]
    fn get_tool_for_action<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(cc_common): Implement proper tool lookup from feature configuration
        // For now, return platform-appropriate tool names
        let tool = match std::env::consts::OS {
            "windows" => {
                let msvc = get_msvc_tool_paths();
                match action_name {
                    "c-compile" | "c++-compile" => {
                        msvc.as_ref().map(|t| t.cl.as_str()).unwrap_or("cl.exe")
                    }
                    "c++-link-executable" | "c++-link-dynamic-library" => {
                        msvc.as_ref().map(|t| t.link.as_str()).unwrap_or("link.exe")
                    }
                    "c++-link-static-library" => {
                        msvc.as_ref().map(|t| t.lib.as_str()).unwrap_or("lib.exe")
                    }
                    "strip" | "objcopy" => "",
                    _ => msvc.as_ref().map(|t| t.cl.as_str()).unwrap_or("cl.exe"),
                }
            }
            "macos" => match action_name {
                "c-compile" => "/usr/bin/clang",
                "c++-compile" => "/usr/bin/clang++",
                "c++-link-executable" | "c++-link-dynamic-library" => "/usr/bin/clang++",
                "c++-link-static-library" => "/usr/bin/ar",
                "strip" => "/usr/bin/strip",
                "objcopy" => "/usr/bin/objcopy",
                _ => "/usr/bin/clang",
            },
            _ => match action_name {
                "c-compile" => "/usr/bin/gcc",
                "c++-compile" => "/usr/bin/g++",
                "c++-link-executable" | "c++-link-dynamic-library" => "/usr/bin/g++",
                "c++-link-static-library" => "/usr/bin/ar",
                "strip" => "/usr/bin/strip",
                "objcopy" => "/usr/bin/objcopy",
                _ => "/usr/bin/gcc",
            },
        };
        Ok(tool.to_owned())
    }

    /// Gets execution requirements for a given action.
    #[allow(unused_variables)]
    fn get_execution_requirements<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // TODO(cc_common): Implement proper execution requirements
        let map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();
        Ok(eval.heap().alloc(Dict::new(map)))
    }

    /// Checks if an action is enabled in the feature configuration.
    ///
    /// In Bazel, action enablement is controlled by features that gate specific
    /// compiler/linker actions. We check if the action_name corresponds to a
    /// known feature and consult the FeatureConfiguration if so.
    fn action_is_enabled<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        // Try to consult the FeatureConfiguration for action-specific features.
        // In Bazel, actions like "c++-compile", "c++-link-executable" etc. are
        // enabled based on the feature configuration. We map action names to
        // features where there's a direct correspondence.
        if let Some(fc) = feature_configuration.downcast_ref::<FeatureConfiguration>() {
            // Some actions correspond directly to features
            let feature_name = match action_name {
                "c++-compile" | "c-compile" | "cc-flags-make-variable" => None, // Always enabled
                "c++-link-executable" | "c++-link-dynamic-library"
                | "c++-link-nodeps-dynamic-library" | "c++-link-static-library" => None, // Always enabled
                // For other action names, check if there's a matching feature
                other => Some(other),
            };
            if let Some(feature) = feature_name {
                return Ok(fc.is_feature_enabled(feature));
            }
        }
        // Default: actions are enabled
        Ok(true)
    }

    /// Gets the command line for an action (memory inefficient version).
    #[allow(unused_variables)]
    fn get_memory_inefficient_command_line<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let mut args: Vec<Value<'v>> = Vec::new();

        // Helper to get a variable value from CcToolchainVariables or dict
        let get_var = |key: &str| -> Option<Value<'v>> {
            if let Ok(Some(v)) = variables.get_attr(key, heap) {
                return Some(v);
            }
            if let Some(dict_ref) = DictRef::from_value(variables) {
                return dict_ref.get_str(key);
            }
            None
        };

        // Helper to iterate a value that may be a depset or list
        let iterate_value =
            |val: Value<'v>, eval_ref: &mut Evaluator<'v, '_, '_>| -> Vec<Value<'v>> {
                let h = eval_ref.heap();
                if let Ok(Some(to_list_method)) = val.get_attr("to_list", h) {
                    if let Ok(list_val) = eval_ref.eval_function(to_list_method, &[], &[]) {
                        if let Ok(iter) = list_val.iterate(h) {
                            return iter.collect();
                        }
                    }
                }
                if let Ok(iter) = val.iterate(h) {
                    iter.collect()
                } else {
                    Vec::new()
                }
            };

        let is_compile = action_name.contains("compile") && !action_name.contains("preprocess");
        let is_static_lib = action_name.contains("static-library");
        let is_dynamic_lib = action_name.contains("dynamic-library");
        let is_link = action_name.contains("link") && !action_name.contains("compile");
        let msvc = is_windows_host();

        // Helper to get string from a value (string or File with .path)
        let get_str_val = |v: Value<'v>| -> Option<String> {
            if let Some(s) = v.unpack_str() {
                return Some(s.to_owned());
            }
            if let Ok(Some(path_val)) = v.get_attr("path", heap) {
                if let Some(path_str) = path_val.unpack_str() {
                    return Some(path_str.to_owned());
                }
            }
            None
        };

        // --- Link/Archive actions ---
        if is_link {
            let output_path = get_var("output_execpath").and_then(|v| get_str_val(v));

            if is_static_lib {
                if msvc {
                    // MSVC: lib.exe /nologo /OUT:<output>
                    args.push(heap.alloc_str("/nologo").to_value());
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str(&format!("/OUT:{}", path)).to_value());
                    }
                } else {
                    // ar archiver: rcs <output>
                    args.push(heap.alloc_str("rcs").to_value());
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str(path).to_value());
                    }
                }
            } else if is_dynamic_lib {
                if msvc {
                    // MSVC: link.exe /nologo /DLL /OUT:<output>
                    args.push(heap.alloc_str("/nologo").to_value());
                    args.push(heap.alloc_str("/DLL").to_value());
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str(&format!("/OUT:{}", path)).to_value());
                    }
                } else {
                    args.push(heap.alloc_str("-shared").to_value());
                    args.push(heap.alloc_str("-fPIC").to_value());
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str("-o").to_value());
                        args.push(heap.alloc_str(path).to_value());
                    }
                }
            } else {
                // Executable link
                if msvc {
                    args.push(heap.alloc_str("/nologo").to_value());
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str(&format!("/OUT:{}", path)).to_value());
                    }
                } else {
                    if let Some(ref path) = output_path {
                        args.push(heap.alloc_str("-o").to_value());
                        args.push(heap.alloc_str(path).to_value());
                    }
                }
            }

            // User link flags
            if let Some(user_flags) = get_var("user_link_flags") {
                if !user_flags.is_none() {
                    for flag in iterate_value(user_flags, eval) {
                        if let Some(s) = flag.unpack_str() {
                            if !s.is_empty() {
                                args.push(heap.alloc_str(s).to_value());
                            }
                        }
                    }
                }
            }

            return Ok(heap.alloc(args));
        }

        // --- Compile actions below ---

        if msvc {
            // MSVC compile flags
            args.push(heap.alloc_str("/nologo").to_value());
            args.push(heap.alloc_str("/EHsc").to_value());
            if is_compile {
                args.push(heap.alloc_str("/c").to_value());
            }
        } else {
            // GCC/Clang: -fPIC if pic variable is set
            if get_var("pic").is_some() {
                args.push(heap.alloc_str("-fPIC").to_value());
            }
            if is_compile {
                args.push(heap.alloc_str("-c").to_value());
            }
        }

        // Source file
        if let Some(source) = get_var("source_file") {
            if !source.is_none() {
                if let Some(s) = source.unpack_str() {
                    args.push(heap.alloc_str(s).to_value());
                } else if let Ok(Some(path_val)) = source.get_attr("path", heap) {
                    if let Some(path_str) = path_val.unpack_str() {
                        args.push(heap.alloc_str(path_str).to_value());
                    } else {
                        args.push(path_val);
                    }
                } else {
                    args.push(source);
                }
            }
        }

        // Output file
        if let Some(output) = get_var("output_file") {
            if !output.is_none() {
                let out_flag = if msvc { "/Fo" } else { "-o" };
                args.push(heap.alloc_str(out_flag).to_value());
                if let Some(s) = output.unpack_str() {
                    args.push(heap.alloc_str(s).to_value());
                } else if let Ok(Some(path_val)) = output.get_attr("path", heap) {
                    if let Some(path_str) = path_val.unpack_str() {
                        args.push(heap.alloc_str(path_str).to_value());
                    } else {
                        args.push(path_val);
                    }
                } else {
                    args.push(output);
                }
            }
        }

        // User compile flags
        if let Some(user_flags) = get_var("user_compile_flags") {
            if !user_flags.is_none() {
                for flag in iterate_value(user_flags, eval) {
                    if let Some(s) = flag.unpack_str() {
                        if !s.is_empty() {
                            args.push(heap.alloc_str(s).to_value());
                        }
                    }
                }
            }
        }

        // Include paths
        let inc_prefix = if msvc { "/I" } else { "-I" };
        if let Some(includes) = get_var("include_paths") {
            if !includes.is_none() {
                for inc in iterate_value(includes, eval) {
                    if let Some(s) = inc.unpack_str() {
                        if !s.is_empty() {
                            args.push(heap.alloc_str(&format!("{}{}", inc_prefix, s)).to_value());
                        }
                    }
                }
            }
        }

        // Quote include paths
        if let Some(quote_includes) = get_var("quote_include_paths") {
            if !quote_includes.is_none() {
                for inc in iterate_value(quote_includes, eval) {
                    if let Some(s) = inc.unpack_str() {
                        if !s.is_empty() {
                            if msvc {
                                args.push(heap.alloc_str(&format!("/I{}", s)).to_value());
                            } else {
                                args.push(heap.alloc_str("-iquote").to_value());
                                args.push(heap.alloc_str(s).to_value());
                            }
                        }
                    }
                }
            }
        }

        // System include paths
        if let Some(system_includes) = get_var("system_include_paths") {
            if !system_includes.is_none() {
                for inc in iterate_value(system_includes, eval) {
                    if let Some(s) = inc.unpack_str() {
                        if !s.is_empty() {
                            if msvc {
                                args.push(heap.alloc_str(&format!("/I{}", s)).to_value());
                            } else {
                                args.push(heap.alloc_str(&format!("-isystem{}", s)).to_value());
                            }
                        }
                    }
                }
            }
        }

        // External include paths
        if let Some(ext_includes) = get_var("external_include_paths") {
            if !ext_includes.is_none() {
                for inc in iterate_value(ext_includes, eval) {
                    if let Some(s) = inc.unpack_str() {
                        if !s.is_empty() {
                            if msvc {
                                args.push(heap.alloc_str(&format!("/I{}", s)).to_value());
                            } else {
                                args.push(heap.alloc_str(&format!("-isystem{}", s)).to_value());
                            }
                        }
                    }
                }
            }
        }

        // Preprocessor defines
        let def_prefix = if msvc { "/D" } else { "-D" };
        if let Some(defines) = get_var("preprocessor_defines") {
            if !defines.is_none() {
                for def in iterate_value(defines, eval) {
                    if let Some(s) = def.unpack_str() {
                        args.push(heap.alloc_str(&format!("{}{}", def_prefix, s)).to_value());
                    }
                }
            }
        }

        Ok(heap.alloc(args))
    }

    /// Gets environment variables for an action.
    #[allow(unused_variables)]
    fn get_environment_variables<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        #[starlark(require = named)] variables: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = (this, feature_configuration, action_name, variables);
        let heap = eval.heap();
        let mut map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();

        // On Windows, provide MSVC environment variables for compilation/linking
        #[cfg(target_os = "windows")]
        if let Some(tools) = get_msvc_tool_paths() {
            let include_val = format!(
                "{};{};{};{}",
                tools.msvc_include, tools.ucrt_include, tools.um_include, tools.shared_include
            );
            map.insert_hashed(
                heap.alloc_str("INCLUDE").to_value().get_hashed().unwrap(),
                heap.alloc_str(&include_val).to_value(),
            );

            let lib_val = format!(
                "{};{};{}",
                tools.msvc_lib, tools.ucrt_lib, tools.um_lib
            );
            map.insert_hashed(
                heap.alloc_str("LIB").to_value().get_hashed().unwrap(),
                heap.alloc_str(&lib_val).to_value(),
            );
        }

        Ok(heap.alloc(Dict::new(map)))
    }

    /// Creates empty toolchain variables.
    fn empty_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Ok(eval.heap().alloc(CcToolchainVariablesGen {
            vars: Value::new_none(),
        }))
    }

    /// Gets legacy CC_FLAGS make variable value.
    #[allow(unused_variables)]
    fn legacy_cc_flags_make_variable_do_not_use<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<String> {
        // TODO(cc_common): Extract from toolchain
        Ok(String::new())
    }

    /// Checks if experimental cc_shared_library is enabled.
    fn check_experimental_cc_shared_library(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Checks if objc_library transition is disabled.
    fn incompatible_disable_objc_library_transition(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Checks if Go exec groups should be added to binary rules.
    fn add_go_exec_groups_to_binary_rules(
        #[starlark(this)] _this: &CcCommonModule,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Checks if implementation_deps is allowed by allowlist.
    #[allow(unused_variables)]
    fn implementation_deps_allowed_by_allowlist<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] ctx: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Creates a compilation action (allowlisted).
    fn create_compile_action<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[allow(unused_variables)] eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<NoneType> {
        // TODO(cc_common): Implement create_compile_action
        Ok(NoneType)
    }

    /// Creates a linker input.
    #[allow(unused_variables)]
    fn create_linker_input<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] owner: Value<'v>,
        #[starlark(require = named, default = NoneType)] libraries: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_link_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] linkstamps: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = (this, linkstamps);
        // Store user_link_flags as-is (depset or list); wrap list in depset if needed
        let user_flags = if user_link_flags.is_none() {
            Value::new_none()
        } else if user_link_flags.request_value::<crate::interpreter::rule_defs::depset::Depset>().is_some() {
            // Already a depset
            user_link_flags
        } else {
            // Wrap list/iterable in a depset
            match user_link_flags.iterate(heap) {
                Ok(iter) => {
                    match crate::interpreter::rule_defs::depset::make_depset_from_lists(
                        heap,
                        iter.collect(),
                        Vec::new(),
                        "default",
                    ) {
                        Ok(ds) => ds,
                        Err(_) => user_link_flags,
                    }
                }
                Err(_) => user_link_flags,
            }
        };
        Ok(heap.alloc(LinkerInputStubGen {
            owner,
            libraries,
            user_link_flags: user_flags,
            additional_inputs,
        }))
    }

    /// Creates a linking context.
    #[allow(unused_variables)]
    fn create_linking_context<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] linker_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] owner: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(LinkingContextWithInputsGen { linker_inputs }))
    }

    /// Checks if a feature is enabled in the feature configuration.
    #[allow(unused_variables)]
    fn is_enabled<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] feature_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<bool> {
        let _ = (this, eval);
        // Try to downcast to our FeatureConfiguration type
        if let Some(fc) = feature_configuration.downcast_ref::<FeatureConfiguration>() {
            return Ok(fc.is_feature_enabled(feature_name));
        }
        // Fallback for non-FeatureConfiguration values (e.g., None passed from tests)
        let enabled = match feature_name {
            "supports_dynamic_linker" | "supports_interface_shared_libraries" => true,
            "pic" | "supports_pic" => !is_windows_host(),
            "targets_windows" => is_windows_host(),
            "static_link_cpp_runtimes" => true,
            _ => false,
        };
        Ok(enabled)
    }

    /// Creates a compilation context from headers, includes, and defines.
    ///
    /// This is used by rules_cc to construct the compilation context that
    /// gets propagated to dependents via CcInfo.
    #[allow(unused_variables)]
    fn create_compilation_context<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] quote_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] system_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] framework_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] local_defines: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_public_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_private_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_textual_headers: Value<'v>,
        #[starlark(require = named, default = NoneType)] purpose: Value<'v>,
        #[starlark(kwargs)] kwargs: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CcCompilationContextGen {
            headers,
            includes,
            quote_includes,
            system_includes,
            framework_includes,
            defines,
            local_defines,
        }))
    }

    /// Creates compilation outputs.
    #[allow(unused_variables)]
    fn create_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] lto_compilation_context: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CompilationOutputsGen {
            objects,
            pic_objects,
        }))
    }

    /// Merges multiple compilation outputs into one.
    #[allow(unused_variables)]
    fn merge_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] compilation_outputs: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Collect all objects and pic_objects from the list of compilation outputs
        let mut all_objects: Vec<Value<'v>> = Vec::new();
        let mut all_pic_objects: Vec<Value<'v>> = Vec::new();

        if !compilation_outputs.is_none() {
            if let Ok(iter) = compilation_outputs.iterate(heap) {
                for co in iter {
                    if let Ok(Some(objects)) = co.get_attr("objects", heap) {
                        if !objects.is_none() {
                            if let Ok(obj_iter) = objects.iterate(heap) {
                                all_objects.extend(obj_iter);
                            }
                        }
                    }
                    if let Ok(Some(pic_objects)) = co.get_attr("pic_objects", heap) {
                        if !pic_objects.is_none() {
                            if let Ok(pic_iter) = pic_objects.iterate(heap) {
                                all_pic_objects.extend(pic_iter);
                            }
                        }
                    }
                }
            }
        }

        Ok(heap.alloc(CompilationOutputsGen {
            objects: if all_objects.is_empty() {
                Value::new_none()
            } else {
                heap.alloc(all_objects)
            },
            pic_objects: if all_pic_objects.is_empty() {
                Value::new_none()
            } else {
                heap.alloc(all_pic_objects)
            },
        }))
    }

    /// Creates a linking context from compilation outputs.
    ///
    /// Returns a tuple of (linking_context, linking_outputs).
    #[allow(unused_variables)]
    fn create_linking_context_from_compilation_outputs<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] name: &str,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named, default = NoneType)] compilation_outputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] user_link_flags: Value<'v>,
        #[starlark(require = named, default = NoneType)] linking_contexts: Value<'v>,
        #[starlark(require = named, default = NoneType)] language: Value<'v>,
        #[starlark(require = named, default = false)] disallow_static_libraries: bool,
        #[starlark(require = named, default = false)] disallow_dynamic_library: bool,
        #[starlark(require = named, default = NoneType)] additional_inputs: Value<'v>,
        #[starlark(require = named, default = NoneType)] grep_includes: Value<'v>,
        #[starlark(require = named, default = NoneType)] stamp: Value<'v>,
        #[starlark(require = named, default = NoneType)] linked_dll_name_suffix: Value<'v>,
        #[starlark(require = named, default = NoneType)] win_def_file: Value<'v>,
        #[starlark(require = named, default = NoneType)] test_only_target: Value<'v>,
        #[starlark(require = named, default = false)] alwayslink: bool,
        #[starlark(require = named, default = NoneType)] variables_extension: Value<'v>,
        #[starlark(require = named, default = NoneType)] main_output: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Create library_to_link from compilation outputs
        let library_to_link = if compilation_outputs.is_none() {
            Value::new_none()
        } else {
            // Extract objects and pic_objects from compilation_outputs
            let objects = compilation_outputs
                .get_attr("objects", heap)
                .ok()
                .flatten()
                .unwrap_or(Value::new_none());
            let pic_objects = compilation_outputs
                .get_attr("pic_objects", heap)
                .ok()
                .flatten()
                .unwrap_or(Value::new_none());
            heap.alloc(LibraryToLinkGen {
                static_library: Value::new_none(),
                pic_static_library: Value::new_none(),
                dynamic_library: Value::new_none(),
                interface_library: Value::new_none(),
                objects,
                pic_objects,
                alwayslink,
            })
        };

        // Create linking outputs
        let linking_outputs = heap.alloc(CcLinkingOutputsGen {
            library_to_link,
            executable: Value::new_none(),
        });

        // Create a LinkerInput wrapping the library_to_link
        let libraries_depset = if library_to_link.is_none() {
            heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())
        } else {
            crate::interpreter::rule_defs::depset::make_depset_from_lists(
                heap,
                vec![library_to_link],
                Vec::new(),
                "default",
            )?
        };

        // Wrap user_link_flags in a depset if provided as a list
        let user_link_flags_depset = if user_link_flags.is_none() {
            heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())
        } else {
            match user_link_flags.iterate(heap) {
                Ok(iter) => {
                    match crate::interpreter::rule_defs::depset::make_depset_from_lists(
                        heap,
                        iter.collect(),
                        Vec::new(),
                        "default",
                    ) {
                        Ok(ds) => ds,
                        Err(_) => user_link_flags,
                    }
                }
                Err(_) => user_link_flags,
            }
        };

        let additional_inputs_depset = if additional_inputs.is_none() {
            heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty())
        } else {
            additional_inputs
        };

        let linker_input = heap.alloc(LinkerInputStubGen {
            owner: Value::new_none(), // No owner label available in this context
            libraries: libraries_depset,
            user_link_flags: user_link_flags_depset,
            additional_inputs: additional_inputs_depset,
        });

        // Create linker_inputs depset containing this LinkerInput
        // Also include transitive linker_inputs from provided linking_contexts
        let mut transitive_depsets: Vec<Value<'v>> = Vec::new();
        if !linking_contexts.is_none() {
            if let Ok(iter) = linking_contexts.iterate(heap) {
                for ctx_val in iter {
                    if let Ok(Some(li)) = ctx_val.get_attr("linker_inputs", heap) {
                        if !li.is_none() {
                            transitive_depsets.push(li);
                        }
                    }
                }
            }
        }

        let linker_inputs = crate::interpreter::rule_defs::depset::make_depset_from_lists(
            heap,
            vec![linker_input],
            transitive_depsets,
            "default",
        )?;

        // Create linking context
        let linking_context = heap.alloc(LinkingContextWithInputsGen { linker_inputs });

        // Return tuple
        Ok(heap.alloc((linking_context, linking_outputs)))
    }

    /// Merges multiple linking contexts into one.
    ///
    /// Collects linker_inputs from all provided linking contexts into a
    /// single merged linking context with transitive depset.
    #[allow(unused_variables)]
    fn merge_linking_contexts<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] linking_contexts: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Collect all linker_inputs depsets as transitive children
        let mut transitive_depsets: Vec<Value<'v>> = Vec::new();

        if !linking_contexts.is_none() {
            if let Ok(iter) = linking_contexts.iterate(heap) {
                for ctx_val in iter {
                    if let Ok(Some(linker_inputs)) = ctx_val.get_attr("linker_inputs", heap) {
                        if !linker_inputs.is_none() {
                            transitive_depsets.push(linker_inputs);
                        }
                    }
                }
            }
        }

        // Create merged depset with all inputs as transitive children
        let linker_inputs = crate::interpreter::rule_defs::depset::make_depset_from_lists(
            heap,
            Vec::new(), // no direct elements
            transitive_depsets,
            "default",
        )?;

        Ok(heap.alloc(LinkingContextWithInputsGen { linker_inputs }))
    }

    /// Creates a library_to_link struct.
    #[allow(unused_variables)]
    fn create_library_to_link<'v>(
        #[starlark(this)] this: &CcCommonModule,
        #[starlark(require = named)] actions: Value<'v>,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = NoneType)] static_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_static_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] dynamic_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] interface_library: Value<'v>,
        #[starlark(require = named, default = NoneType)] pic_objects: Value<'v>,
        #[starlark(require = named, default = NoneType)] objects: Value<'v>,
        #[starlark(require = named, default = false)] alwayslink: bool,
        #[starlark(require = named, default = NoneType)] dynamic_library_symlink_path: Value<'v>,
        #[starlark(require = named, default = NoneType)] interface_library_symlink_path: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(LibraryToLinkGen {
            static_library,
            pic_static_library,
            dynamic_library,
            interface_library,
            objects,
            pic_objects,
            alwayslink,
        }))
    }

    /// Returns tool execution requirements for an action.
    ///
    /// Returns a list of execution requirements (strings like "requires-network")
    /// that should be added to actions using the specified tool.
    #[allow(unused_variables)]
    fn get_tool_requirement_for_action<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] action_name: &str,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Return an empty list - no special execution requirements
        Ok(eval.heap().alloc(Vec::<String>::new()))
    }

    /// Creates compile variables for use with get_memory_inefficient_command_line.
    ///
    /// Returns CcToolchainVariables with compilation-related settings.
    #[allow(unused_variables)]
    fn create_compile_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        source_file: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        output_file: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        user_compile_flags: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        include_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        quote_include_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        system_include_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        framework_include_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        preprocessor_defines: Value<'v>,
        #[starlark(require = named, default = false)] use_pic: bool,
        #[starlark(require = named, default = false)] add_legacy_cxx_options: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        variables_extension: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        let mut map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();

        if !source_file.is_none() {
            map.insert_hashed(
                heap.alloc_str("source_file").to_value().get_hashed().unwrap(),
                source_file,
            );
        }
        if !output_file.is_none() {
            map.insert_hashed(
                heap.alloc_str("output_file").to_value().get_hashed().unwrap(),
                output_file,
            );
        }
        if !user_compile_flags.is_none() {
            map.insert_hashed(
                heap.alloc_str("user_compile_flags").to_value().get_hashed().unwrap(),
                user_compile_flags,
            );
        }
        if !include_directories.is_none() {
            map.insert_hashed(
                heap.alloc_str("include_directories").to_value().get_hashed().unwrap(),
                include_directories,
            );
        }
        if !preprocessor_defines.is_none() {
            map.insert_hashed(
                heap.alloc_str("preprocessor_defines").to_value().get_hashed().unwrap(),
                preprocessor_defines,
            );
        }
        if use_pic {
            map.insert_hashed(
                heap.alloc_str("use_pic").to_value().get_hashed().unwrap(),
                Value::new_bool(true),
            );
        }

        let vars = heap.alloc(Dict::new(map));
        Ok(heap.alloc(CcToolchainVariablesGen { vars }))
    }

    /// Creates link variables for use with get_memory_inefficient_command_line.
    ///
    /// Used by rules_rust to get linker command line from cc toolchain.
    #[allow(unused_variables)]
    fn create_link_variables<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named)] feature_configuration: Value<'v>,
        #[starlark(require = named)] cc_toolchain: Value<'v>,
        #[starlark(require = named, default = false)] is_linking_dynamic_library: bool,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        runtime_library_search_directories: Value<'v>,
        #[starlark(require = named, default = starlark::values::none::NoneType)]
        user_link_flags: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = (feature_configuration, cc_toolchain);
        let heap = eval.heap();
        // Build a dict with the link variables for get_memory_inefficient_command_line
        let mut map: SmallMap<Value<'v>, Value<'v>> = SmallMap::new();

        if !user_link_flags.is_none() {
            map.insert_hashed(
                heap.alloc_str("user_link_flags").to_value().get_hashed().unwrap(),
                user_link_flags,
            );
        }

        if !runtime_library_search_directories.is_none() {
            map.insert_hashed(
                heap.alloc_str("runtime_library_search_directories")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                runtime_library_search_directories,
            );
        }

        if is_linking_dynamic_library {
            map.insert_hashed(
                heap.alloc_str("is_linking_dynamic_library")
                    .to_value()
                    .get_hashed()
                    .unwrap(),
                Value::new_bool(true),
            );
        }

        let vars = heap.alloc(Dict::new(map));
        Ok(heap.alloc(CcToolchainVariablesGen { vars }))
    }

    /// Merges multiple CcInfo providers into a single CcInfo.
    ///
    /// Collects compilation contexts and linking contexts from all input
    /// CcInfo providers and merges them into a single CcInfo.
    #[allow(unused_variables)]
    fn merge_cc_infos<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = named, default = NoneType)] cc_infos: Value<'v>,
        #[starlark(require = named, default = NoneType)] direct_cc_infos: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        // Collect compilation contexts and linking contexts from all inputs
        let mut linking_contexts: Vec<Value<'v>> = Vec::new();
        let mut headers_depsets: Vec<Value<'v>> = Vec::new();
        let mut includes_depsets: Vec<Value<'v>> = Vec::new();
        let mut defines_depsets: Vec<Value<'v>> = Vec::new();

        // Helper closure to extract contexts from a CcInfo
        let mut process_info = |info: Value<'v>| {
            if let Ok(Some(comp_ctx)) = info.get_attr("compilation_context", heap) {
                if !comp_ctx.is_none() {
                    // Extract headers, includes, defines depsets from compilation context
                    if let Ok(Some(h)) = comp_ctx.get_attr("headers", heap) {
                        if !h.is_none() {
                            headers_depsets.push(h);
                        }
                    }
                    if let Ok(Some(i)) = comp_ctx.get_attr("includes", heap) {
                        if !i.is_none() {
                            includes_depsets.push(i);
                        }
                    }
                    if let Ok(Some(d)) = comp_ctx.get_attr("defines", heap) {
                        if !d.is_none() {
                            defines_depsets.push(d);
                        }
                    }
                }
            }
            if let Ok(Some(link_ctx)) = info.get_attr("linking_context", heap) {
                if !link_ctx.is_none() {
                    linking_contexts.push(link_ctx);
                }
            }
        };

        // Process cc_infos (transitive)
        if !cc_infos.is_none() {
            if let Ok(iter) = cc_infos.iterate(heap) {
                for info in iter {
                    process_info(info);
                }
            }
        }

        // Process direct_cc_infos
        if !direct_cc_infos.is_none() {
            if let Ok(iter) = direct_cc_infos.iterate(heap) {
                for info in iter {
                    process_info(info);
                }
            }
        }

        // Merge compilation contexts by combining headers/includes/defines depsets
        let merged_compilation_context = if headers_depsets.is_empty()
            && includes_depsets.is_empty()
            && defines_depsets.is_empty()
        {
            Value::new_none()
        } else {
            let merged_headers = if headers_depsets.is_empty() {
                Value::new_none()
            } else {
                crate::interpreter::rule_defs::depset::make_depset_from_lists(
                    heap,
                    Vec::new(),
                    headers_depsets,
                    "default",
                )?
            };
            let merged_includes = if includes_depsets.is_empty() {
                Value::new_none()
            } else {
                crate::interpreter::rule_defs::depset::make_depset_from_lists(
                    heap,
                    Vec::new(),
                    includes_depsets,
                    "default",
                )?
            };
            let merged_defines = if defines_depsets.is_empty() {
                Value::new_none()
            } else {
                crate::interpreter::rule_defs::depset::make_depset_from_lists(
                    heap,
                    Vec::new(),
                    defines_depsets,
                    "default",
                )?
            };
            heap.alloc(CcCompilationContextGen {
                headers: merged_headers,
                includes: merged_includes,
                quote_includes: Value::new_none(),
                system_includes: Value::new_none(),
                framework_includes: Value::new_none(),
                defines: merged_defines,
                local_defines: Value::new_none(),
            })
        };

        // Merge linking contexts into a single one
        let merged_linking_context = if linking_contexts.is_empty() {
            Value::new_none()
        } else {
            // Collect all linker_inputs depsets as transitive children
            let mut transitive_depsets: Vec<Value<'v>> = Vec::new();
            for ctx_val in &linking_contexts {
                if let Ok(Some(linker_inputs)) = ctx_val.get_attr("linker_inputs", heap) {
                    if !linker_inputs.is_none() {
                        transitive_depsets.push(linker_inputs);
                    }
                }
            }
            let merged_linker_inputs = crate::interpreter::rule_defs::depset::make_depset_from_lists(
                heap,
                Vec::new(),
                transitive_depsets,
                "default",
            )?;
            heap.alloc(LinkingContextWithInputsGen {
                linker_inputs: merged_linker_inputs,
            })
        };

        Ok(heap.alloc(CcInfoInstanceGen {
            compilation_context: merged_compilation_context,
            linking_context: merged_linking_context,
        }))
    }

    /// Creates a debug context from compilation outputs.
    #[allow(unused_variables)]
    fn create_debug_context<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = pos, default = NoneType)] compilation_outputs: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CcDebugContext))
    }

    /// Merges multiple debug contexts into one.
    #[allow(unused_variables)]
    fn merge_debug_context<'v>(
        #[starlark(this)] _this: &CcCommonModule,
        #[starlark(require = pos, default = NoneType)] debug_contexts: Value<'v>,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(CcDebugContext))
    }
}

// ============================================================================
// CompilationOutputs - Outputs from C++ compilation
// ============================================================================

/// CompilationOutputs holds the output files from C++ compilation.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct CompilationOutputsGen<V: ValueLifetimeless> {
    objects: V,
    pic_objects: V,
}

starlark_complex_value!(pub CompilationOutputs);

impl<V: ValueLifetimeless> Display for CompilationOutputsGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<CompilationOutputs>")
    }
}

/// Methods on CompilationOutputs for accessing coverage files.
#[starlark_module]
fn compilation_outputs_methods(builder: &mut MethodsBuilder) {
    /// Returns coverage (gcno) files from non-PIC compilation.
    fn gcno_files<'v>(this: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return empty list - no coverage files in this stub
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Returns coverage (gcno) files from PIC compilation.
    fn pic_gcno_files<'v>(this: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        // Return empty list - no coverage files in this stub
        Ok(heap.alloc(AllocList::EMPTY))
    }
}

#[starlark::values::starlark_value(type = "CompilationOutputs")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CompilationOutputsGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "objects" | "pic_objects")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "objects" => Some(self.objects.to_value()),
            "pic_objects" => Some(self.pic_objects.to_value()),
            // These are additional attributes that rules_cc may access
            "_gcno_files" | "_pic_gcno_files" => Some(heap.alloc(AllocList::EMPTY)),
            _ => None,
        }
    }

    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(compilation_outputs_methods)
    }
}

// ============================================================================
// LibraryToLink - A library artifact for linking
// ============================================================================

/// LibraryToLink represents a library that can be linked.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct LibraryToLinkGen<V: ValueLifetimeless> {
    static_library: V,
    pic_static_library: V,
    dynamic_library: V,
    interface_library: V,
    objects: V,
    pic_objects: V,
    alwayslink: bool,
}

starlark_complex_value!(pub LibraryToLink);

impl<V: ValueLifetimeless> Display for LibraryToLinkGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<LibraryToLink>")
    }
}

#[starlark::values::starlark_value(type = "LibraryToLink")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LibraryToLinkGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "static_library"
                | "pic_static_library"
                | "dynamic_library"
                | "interface_library"
                | "objects"
                | "pic_objects"
                | "alwayslink"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "static_library" => Some(self.static_library.to_value()),
            "pic_static_library" => Some(self.pic_static_library.to_value()),
            "dynamic_library" => Some(self.dynamic_library.to_value()),
            "interface_library" => Some(self.interface_library.to_value()),
            "objects" => {
                if self.objects.to_value().is_none() {
                    Some(heap.alloc(starlark::values::list::AllocList::EMPTY))
                } else {
                    Some(self.objects.to_value())
                }
            }
            "pic_objects" => {
                if self.pic_objects.to_value().is_none() {
                    Some(heap.alloc(starlark::values::list::AllocList::EMPTY))
                } else {
                    Some(self.pic_objects.to_value())
                }
            }
            "alwayslink" => Some(Value::new_bool(self.alwayslink)),
            _ => None,
        }
    }
}

// ============================================================================
// CcLinkingOutputs - Outputs from linking
// ============================================================================

/// CcLinkingOutputs holds the output files from C++ linking.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct CcLinkingOutputsGen<V: ValueLifetimeless> {
    library_to_link: V,
    executable: V,
}

starlark_complex_value!(pub CcLinkingOutputs);

impl<V: ValueLifetimeless> Display for CcLinkingOutputsGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<CcLinkingOutputs>")
    }
}

#[starlark::values::starlark_value(type = "CcLinkingOutputs")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcLinkingOutputsGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "library_to_link" | "executable")
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "library_to_link" => Some(self.library_to_link.to_value()),
            "executable" => Some(self.executable.to_value()),
            _ => None,
        }
    }
}

// ============================================================================
// CcToolchainInfoProvider - Provider for C++ toolchain information
// ============================================================================

/// CcToolchainInfo provider for C++ toolchain information.
///
/// This provider carries toolchain configuration like compiler paths,
/// flags, and supported features.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcToolchainInfoProvider;

impl CcToolchainInfoProvider {
    /// Get the static provider ID for CcToolchainInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "CcToolchainInfo".to_owned(),
            })
        })
    }
}

impl Display for CcToolchainInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcToolchainInfo>")
    }
}

starlark_simple_value!(CcToolchainInfoProvider);

impl ProviderCallableLike for CcToolchainInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "CcToolchainInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainInfoProvider {
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

// ============================================================================
// CcInfo provider - C++ compilation/linking information
// ============================================================================

/// CcInfo provider callable - contains C++ compilation and linking information.
///
/// In Bazel 9.0+, CcInfo is actually defined in pure Starlark in rules_cc
/// (cc/private/cc_info.bzl). This native stub exists for compatibility with
/// code that references the native CcInfo before rules_cc is loaded.
///
/// Implements ProviderCallableLike so it can be used as `CcInfo in dep` and
/// `dep[CcInfo]` for provider collection lookups.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcInfoProvider;

impl CcInfoProvider {
    /// Get the static provider ID for CcInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "CcInfo".to_owned(),
            })
        })
    }
}

impl Display for CcInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcInfo>")
    }
}

starlark_simple_value!(CcInfoProvider);

impl ProviderCallableLike for CcInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "CcInfo")]
impl<'v> StarlarkValue<'v> for CcInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // CcInfo(compilation_context=..., linking_context=...)
        let kwargs = args.names_map()?;
        let heap = eval.heap();
        let compilation_context = kwargs
            .get("compilation_context")
            .copied()
            .unwrap_or(Value::new_none());
        let linking_context = kwargs
            .get("linking_context")
            .copied()
            .unwrap_or(Value::new_none());
        Ok(heap.alloc(CcInfoInstanceGen {
            compilation_context,
            linking_context,
        }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// A CcInfo instance with actual compilation and linking context data.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct CcInfoInstanceGen<V: ValueLifetimeless> {
    compilation_context: V,
    linking_context: V,
}

starlark_complex_value!(pub CcInfoInstance);

impl<V: ValueLifetimeless> Display for CcInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for CcInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn id(&self) -> &Arc<ProviderId> {
        CcInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![
            ("compilation_context", self.compilation_context.to_value()),
            ("linking_context", self.linking_context.to_value()),
        ]
    }
}

#[starlark::values::starlark_value(type = "CcInfoInstance")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for CcInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "compilation_context"
                | "linking_context"
                | "_legacy_transitive_native_libraries"
                | "_debug_context"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "compilation_context" => {
                if self.compilation_context.to_value().is_none() {
                    use crate::interpreter::rule_defs::context::CompilationContextStub;
                    Some(heap.alloc(CompilationContextStub))
                } else {
                    Some(self.compilation_context.to_value())
                }
            }
            "linking_context" => {
                if self.linking_context.to_value().is_none() {
                    use crate::interpreter::rule_defs::context::LinkingContextStub;
                    Some(heap.alloc(LinkingContextStub))
                } else {
                    Some(self.linking_context.to_value())
                }
            }
            "_legacy_transitive_native_libraries" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_debug_context" => Some(heap.alloc(CcDebugContext)),
            _ => None,
        }
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

/// A stub CcInfo instance (returned when CcInfo(...) is called with no data).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcInfoInstanceStub;

impl Display for CcInfoInstanceStub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcInfo(...)")
    }
}

starlark_simple_value!(CcInfoInstanceStub);

impl<'v> ProviderLike<'v> for CcInfoInstanceStub {
    fn id(&self) -> &Arc<ProviderId> {
        CcInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        Vec::new()
    }
}

#[starlark_value(type = "CcInfoInstance")]
impl<'v> StarlarkValue<'v> for CcInfoInstanceStub {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cc_info_instance_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "compilation_context"
                | "linking_context"
                | "_legacy_transitive_native_libraries"
                | "_debug_context"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        use crate::interpreter::rule_defs::context::CompilationContextStub;
        use crate::interpreter::rule_defs::context::LinkingContextStub;
        match attribute {
            "compilation_context" => Some(heap.alloc(CompilationContextStub)),
            "linking_context" => Some(heap.alloc(LinkingContextStub)),
            "_legacy_transitive_native_libraries" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            "_debug_context" => Some(heap.alloc(CcDebugContext)),
            _ => None,
        }
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

#[starlark_module]
fn cc_info_instance_methods(builder: &mut MethodsBuilder) {
    /// Returns transitive native libraries as a depset.
    fn transitive_native_libraries<'v>(
        this: &CcInfoInstanceStub,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
    }

    /// Returns the debug context for this CcInfo.
    fn debug_context<'v>(this: &CcInfoInstanceStub, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        let _ = this;
        Ok(heap.alloc(CcDebugContext))
    }
}

// ============================================================================
// CcDebugContext - Debug context stub
// ============================================================================

/// Stub for Bazel's CcDebugContext, returned by cc_common.create_debug_context()
/// and cc_common.merge_debug_context().
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcDebugContext;

impl Display for CcDebugContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CcDebugContext()")
    }
}

starlark_simple_value!(CcDebugContext);

#[starlark_value(type = "CcDebugContext")]
impl<'v> StarlarkValue<'v> for CcDebugContext {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "files" | "pic_files")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "files" | "pic_files" => {
                Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
            }
            _ => None,
        }
    }
}

// ============================================================================
// DebugPackageInfo - Debug information provider
// ============================================================================

/// DebugPackageInfo provider for debug/symbol information.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct DebugPackageInfoProvider;

impl Display for DebugPackageInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider DebugPackageInfo>")
    }
}

starlark_simple_value!(DebugPackageInfoProvider);

#[starlark_value(type = "DebugPackageInfo")]
impl<'v> StarlarkValue<'v> for DebugPackageInfoProvider {}

// ============================================================================
// CcSharedLibraryInfo - Shared library information provider
// ============================================================================

/// CcSharedLibraryInfo provider for shared library information.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcSharedLibraryInfoProvider;

impl Display for CcSharedLibraryInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcSharedLibraryInfo>")
    }
}

starlark_simple_value!(CcSharedLibraryInfoProvider);

#[starlark_value(type = "CcSharedLibraryInfo")]
impl<'v> StarlarkValue<'v> for CcSharedLibraryInfoProvider {}

// ============================================================================
// CcToolchainConfigInfo - Toolchain configuration provider
// ============================================================================

/// CcToolchainConfigInfo provider for C++ toolchain configuration.
///
/// This provider carries the full toolchain configuration including
/// compiler paths, feature flags, and action configs. Created by
/// cc_common.create_cc_toolchain_config_info().
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CcToolchainConfigInfoProvider;

// ============================================================================
// OutputGroupInfo - Bazel output groups provider
// ============================================================================

/// OutputGroupInfo provider for grouping outputs.
///
/// This provider is used by rules to specify different groups of outputs
/// for different purposes (e.g., IDE support, coverage, etc.).
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct OutputGroupInfoProvider;

impl OutputGroupInfoProvider {
    /// Get the static provider ID for OutputGroupInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "OutputGroupInfo".to_owned(),
            })
        })
    }
}

impl Display for OutputGroupInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider OutputGroupInfo>")
    }
}

starlark_simple_value!(OutputGroupInfoProvider);

impl ProviderCallableLike for OutputGroupInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "OutputGroupInfo")]
impl<'v> StarlarkValue<'v> for OutputGroupInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &starlark::eval::Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let heap = eval.heap();
        // Get kwargs from arguments
        let kwargs = args.names_map()?;
        // Create a dict from the kwargs using AllocDict
        let groups = heap.alloc(starlark::values::dict::AllocDict(
            kwargs.into_iter().map(|(k, v)| (k.as_str(), v)),
        ));
        Ok(heap.alloc(OutputGroupInfoInstanceGen { groups }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of OutputGroupInfo containing output groups.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct OutputGroupInfoInstanceGen<V: ValueLifetimeless> {
    /// The groups as a dict value
    groups: V,
}

starlark_complex_value!(pub OutputGroupInfoInstance);

impl<V: ValueLifetimeless> Display for OutputGroupInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OutputGroupInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for OutputGroupInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        OutputGroupInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        // OutputGroupInfo doesn't have fixed fields - it has dynamic output groups
        // Return empty for now since the groups are stored in a dict
        vec![]
    }
}

#[starlark::values::starlark_value(type = "OutputGroupInfo")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for OutputGroupInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, heap: Heap<'v>) -> bool {
        // Check if attribute exists in groups dict by trying to iterate
        if let Ok(iter) = self.groups.to_value().iterate(heap) {
            for key in iter {
                if key.unpack_str() == Some(attribute) {
                    return true;
                }
            }
        }
        false
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        // Get attribute from groups dict using at2
        let key = heap.alloc_str(attribute);
        self.groups.to_value().at(key.to_value(), heap).ok()
    }

    // Support 'in' operator: `"key" in output_group_info`
    // Delegates to the underlying groups dict.
    fn is_in(&self, other: Value<'v>) -> starlark::Result<bool> {
        // self.groups.to_value().is_in(other) checks "is other in groups dict"
        self.groups.to_value().is_in(other)
    }

    fn at(&self, index: Value<'v>, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        // Index into groups dict
        self.groups.to_value().at(index, heap)
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }
}

impl Display for CcToolchainConfigInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider CcToolchainConfigInfo>")
    }
}

starlark_simple_value!(CcToolchainConfigInfoProvider);

#[starlark_value(type = "CcToolchainConfigInfo")]
impl<'v> StarlarkValue<'v> for CcToolchainConfigInfoProvider {}

// ============================================================================
// Registration
// ============================================================================

/// Register the cc_common global and related providers.
///
/// Note: Per Bazel's CcRules.java, some providers are set to None because
/// they are defined in Starlark by rules_cc.
#[starlark_module]
pub fn register_cc_common(globals: &mut GlobalsBuilder) {
    /// The cc_common module provides C/C++ compilation support.
    const cc_common: CcCommonModule = CcCommonModule;

    /// CcInfo provider for C++ compilation/linking information.
    const CcInfo: CcInfoProvider = CcInfoProvider;

    /// CcToolchainInfo provider for C++ toolchain information.
    const CcToolchainInfo: CcToolchainInfoProvider = CcToolchainInfoProvider;

    /// CcToolchainConfigInfo provider for toolchain configuration.
    /// Used by cc_common.create_cc_toolchain_config_info().
    const CcToolchainConfigInfo: CcToolchainConfigInfoProvider = CcToolchainConfigInfoProvider;

    /// DebugPackageInfo - None placeholder. Actual provider defined in rules_cc Starlark.
    const DebugPackageInfo: NoneType = NoneType;

    /// CcSharedLibraryInfo - None placeholder. Actual provider defined in rules_cc Starlark.
    const CcSharedLibraryInfo: NoneType = NoneType;

    /// OutputGroupInfo - provider for grouping outputs.
    /// This is callable to create instances.
    const OutputGroupInfo: OutputGroupInfoProvider = OutputGroupInfoProvider;

    /// PackageSpecificationInfo - None placeholder.
    /// This is a Bazel built-in provider for package visibility/allowlisting.
    /// Used by cc_toolchain.bzl for visibility_public_presubmit attribute.
    const PackageSpecificationInfo: NoneType = NoneType;

    /// RunEnvironmentInfo - Provider for specifying environment variables
    /// that should be set when running binaries or tests.
    ///
    /// Usage in rules:
    /// ```python
    /// return [RunEnvironmentInfo(environment = {"FOO": "bar"}, inherited_environment = ["PATH"])]
    /// ```
    const RunEnvironmentInfo: RunEnvironmentInfoProvider = RunEnvironmentInfoProvider;

    /// testing module constant for Bazel-compatible testing utilities.
    /// Currently a stub that provides TestEnvironment.
    const testing: TestingModule = TestingModule;
}

// ============================================================================
// TestingModule - Bazel's testing module
// ============================================================================

/// Stub for the testing module.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct TestingModule;

impl Display for TestingModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<module: testing>")
    }
}

starlark_simple_value!(TestingModule);

#[starlark_value(type = "testing")]
impl<'v> StarlarkValue<'v> for TestingModule {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(testing_methods)
    }
}

#[starlark_module]
fn testing_methods(builder: &mut MethodsBuilder) {
    /// TestEnvironment provider for specifying test environment variables.
    /// This is an alias for RunEnvironmentInfo (deprecated in Bazel).
    #[starlark(attribute)]
    fn TestEnvironment<'v>(this: &TestingModule) -> starlark::Result<RunEnvironmentInfoProvider> {
        let _ = this;
        Ok(RunEnvironmentInfoProvider)
    }

    /// ExecutionInfo provider for specifying execution requirements.
    ///
    /// `testing.ExecutionInfo` is a provider callable. Usage:
    /// ```python
    /// return [testing.ExecutionInfo(requirements = {"no-remote": "1"})]
    /// ```
    /// See: https://bazel.build/rules/lib/providers/ExecutionInfo
    #[starlark(attribute)]
    fn ExecutionInfo(this: &TestingModule) -> starlark::Result<ExecutionInfoProvider> {
        let _ = this;
        Ok(ExecutionInfoProvider)
    }

    /// Creates an analysis test rule or registers an analysis test target.
    ///
    /// In Bazel, `testing.analysis_test(implementation, attrs, ...)` creates a
    /// rule for analysis-time tests. When called with `name` and `attr_values`,
    /// it also registers the target.
    ///
    /// Typical usage (bazel_skylib analysistest.make pattern):
    /// ```python
    /// # In .bzl file - returns a callable rule:
    /// my_test = testing.analysis_test(implementation = _impl, attrs = {...})
    ///
    /// # In BUILD file - registers a test target:
    /// my_test(name = "my_test", target_under_test = ":some_target")
    /// ```
    ///
    /// See: https://bazel.build/rules/lib/builtins/testing#analysis_test
    fn analysis_test<'v>(
        this: &TestingModule,
        #[starlark(require = named)] implementation: Value<'v>,
        #[starlark(require = named, default = NoneType)] name: Value<'v>,
        #[starlark(kwargs)] _kwargs: Value<'v>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        let _ = this;
        if let Some(name_str) = name.unpack_str() {
            // Called with name= directly - register the target
            if let Ok(register) = ANALYSIS_TEST_REGISTER.get() {
                register(eval, name_str)?;
            }
            return Ok(Value::new_none());
        }
        // No name provided - return a callable that registers a target when called
        Ok(eval.heap().alloc(AnalysisTestCallable { implementation }))
    }
}

// ============================================================================
// ExecutionInfo - Provider for specifying test execution requirements
// ============================================================================

/// ExecutionInfo provider callable.
///
/// `testing.ExecutionInfo` is a provider callable that specifies execution
/// requirements for tests. Rules return instances of this provider to declare
/// that their tests need specific execution environment settings.
///
/// Reference: https://bazel.build/rules/lib/providers/ExecutionInfo
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ExecutionInfoProvider;

impl ExecutionInfoProvider {
    /// Get the static provider ID for ExecutionInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "ExecutionInfo".to_owned(),
            })
        })
    }
}

impl Display for ExecutionInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<provider ExecutionInfo>")
    }
}

starlark_simple_value!(ExecutionInfoProvider);

impl ProviderCallableLike for ExecutionInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(Self::provider_id())
    }
}

#[starlark_value(type = "ExecutionInfo")]
impl<'v> StarlarkValue<'v> for ExecutionInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        use starlark::values::dict::AllocDict;
        // Extract requirements kwarg, default to empty dict
        let kwargs = args.names_map()?;
        let requirements = kwargs
            .iter()
            .find_map(|(k, v)| {
                if k.as_str() == "requirements" {
                    Some(*v)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                eval.heap()
                    .alloc(AllocDict(std::iter::empty::<(&str, Value)>()))
            });
        Ok(eval.heap().alloc(ExecutionInfoInstance { requirements }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

/// An instance of ExecutionInfo created by `testing.ExecutionInfo(requirements = {...})`.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    starlark::values::Trace,
    starlark::coerce::Coerce,
    starlark::values::Freeze
)]
#[repr(C)]
pub struct ExecutionInfoInstanceGen<V: ValueLifetimeless> {
    /// Requirements dict mapping string keys to string values.
    requirements: V,
}

starlark_complex_value!(pub ExecutionInfoInstance);

impl<V: ValueLifetimeless> Display for ExecutionInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExecutionInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for ExecutionInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        ExecutionInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![("requirements", self.requirements.to_value())]
    }
}

#[starlark_value(type = "ExecutionInfo")]
impl<'v, V: ValueLike<'v> + 'v> StarlarkValue<'v> for ExecutionInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = ExecutionInfoInstance<'v>;

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "requirements" => Some(self.requirements.to_value()),
            _ => None,
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "requirements")
    }
}

// ============================================================================
// RunEnvironmentInfo - Provider for specifying run/test environment variables
// ============================================================================

/// RunEnvironmentInfo provider callable.
///
/// `RunEnvironmentInfo` is a provider for specifying environment variables
/// to be set when running binaries (`kuro run`) or tests (`kuro test`).
///
/// Reference: https://bazel.build/rules/lib/providers/RunEnvironmentInfo
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct RunEnvironmentInfoProvider;

impl RunEnvironmentInfoProvider {
    /// Get the static provider ID for RunEnvironmentInfo.
    pub fn provider_id() -> &'static Arc<ProviderId> {
        static PROVIDER_ID: OnceLock<Arc<ProviderId>> = OnceLock::new();
        PROVIDER_ID.get_or_init(|| {
            Arc::new(ProviderId {
                path: None,
                name: "RunEnvironmentInfo".to_owned(),
            })
        })
    }
}

impl Display for RunEnvironmentInfoProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RunEnvironmentInfo")
    }
}

starlark_simple_value!(RunEnvironmentInfoProvider);

#[starlark_value(type = "RunEnvironmentInfo")]
impl<'v> StarlarkValue<'v> for RunEnvironmentInfoProvider {
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        use starlark::values::dict::AllocDict;
        let kwargs = args.names_map()?;
        let environment = kwargs
            .iter()
            .find_map(|(k, v)| if k.as_str() == "environment" { Some(*v) } else { None })
            .unwrap_or_else(|| {
                eval.heap()
                    .alloc(AllocDict(std::iter::empty::<(&str, Value)>()))
            });
        let inherited_environment = kwargs
            .iter()
            .find_map(|(k, v)| {
                if k.as_str() == "inherited_environment" {
                    Some(*v)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| eval.heap().alloc(AllocList::EMPTY));
        Ok(eval.heap().alloc(RunEnvironmentInfoInstance {
            environment,
            inherited_environment,
        }))
    }

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderCallableLike>(self);
    }
}

impl ProviderCallableLike for RunEnvironmentInfoProvider {
    fn id(&self) -> kuro_error::Result<&Arc<ProviderId>> {
        Ok(RunEnvironmentInfoProvider::provider_id())
    }
}

/// An instance of RunEnvironmentInfo.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    starlark::values::Trace,
    starlark::coerce::Coerce,
    starlark::values::Freeze
)]
#[repr(C)]
pub struct RunEnvironmentInfoInstanceGen<V: ValueLifetimeless> {
    /// Environment variable dict mapping string keys to string values.
    environment: V,
    /// List of environment variable names to inherit from the host.
    inherited_environment: V,
}

starlark_complex_value!(pub RunEnvironmentInfoInstance);

impl<V: ValueLifetimeless> Display for RunEnvironmentInfoInstanceGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RunEnvironmentInfo(...)")
    }
}

impl<'v, V: ValueLike<'v>> ProviderLike<'v> for RunEnvironmentInfoInstanceGen<V>
where
    Self: fmt::Debug,
{
    fn id(&self) -> &Arc<ProviderId> {
        RunEnvironmentInfoProvider::provider_id()
    }

    fn items(&self) -> Vec<(&str, Value<'v>)> {
        vec![
            ("environment", self.environment.to_value()),
            ("inherited_environment", self.inherited_environment.to_value()),
        ]
    }
}

#[starlark_value(type = "RunEnvironmentInfo")]
impl<'v, V: ValueLike<'v> + 'v> StarlarkValue<'v> for RunEnvironmentInfoInstanceGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    type Canonical = RunEnvironmentInfoInstance<'v>;

    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn ProviderLike>(self);
    }

    fn get_attr(&self, attribute: &str, _heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "environment" => Some(self.environment.to_value()),
            "inherited_environment" => Some(self.inherited_environment.to_value()),
            _ => None,
        }
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "environment" | "inherited_environment")
    }
}

// ============================================================================
// ANALYSIS_TEST_REGISTER - Late binding for registering analysis test targets
// ============================================================================

/// Late binding for the function that registers a native analysis test target.
/// Initialized in kuro_interpreter_for_build to avoid circular dependency.
/// Signature: (eval, target_name) -> starlark::Result<NoneType>
pub static ANALYSIS_TEST_REGISTER: LateBinding<
    for<'v, 'a, 'e> fn(&mut Evaluator<'v, 'a, 'e>, &str) -> starlark::Result<NoneType>,
> = LateBinding::new("ANALYSIS_TEST_REGISTER");

#[derive(Debug, kuro_error::Error)]
#[kuro(tag = Input)]
enum AnalysisTestError {
    #[error("analysis_test_rule can only be invoked after the module is frozen")]
    InvokedBeforeFreezing,
    #[error("analysis_test requires a 'name' argument")]
    MissingName,
    #[error("analysis_test 'name' argument must be a string")]
    NameNotString,
}

// ============================================================================
// AnalysisTestCallable - Returned by testing.analysis_test() when no name given
// ============================================================================

/// A callable that, when invoked with name=..., registers a native analysis test target.
/// Returned by testing.analysis_test() when no `name` argument is provided.
#[derive(Debug, ProvidesStaticType, Trace, NoSerialize, Allocative)]
pub struct AnalysisTestCallable<'v> {
    /// The Starlark implementation function (stored for potential future use).
    implementation: Value<'v>,
}

impl<'v> Display for AnalysisTestCallable<'v> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<analysis_test_rule>")
    }
}

impl<'v> starlark::values::AllocValue<'v> for AnalysisTestCallable<'v> {
    fn alloc_value(self, heap: starlark::values::Heap<'v>) -> Value<'v> {
        heap.alloc_complex(self)
    }
}

impl<'v> Freeze for AnalysisTestCallable<'v> {
    type Frozen = FrozenAnalysisTestCallable;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<FrozenAnalysisTestCallable> {
        Ok(FrozenAnalysisTestCallable {
            implementation: self.implementation.freeze(freezer)?,
        })
    }
}

#[starlark_value(type = "analysis_test_rule")]
impl<'v> StarlarkValue<'v> for AnalysisTestCallable<'v> {
    fn invoke(
        &self,
        _me: Value<'v>,
        _args: &Arguments<'v, '_>,
        _eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        Err(kuro_error::Error::from(AnalysisTestError::InvokedBeforeFreezing).into())
    }
}

/// Frozen version of AnalysisTestCallable.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct FrozenAnalysisTestCallable {
    /// The frozen Starlark implementation function.
    implementation: FrozenValue,
}

impl Display for FrozenAnalysisTestCallable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<analysis_test_rule>")
    }
}

starlark_simple_value!(FrozenAnalysisTestCallable);

#[starlark_value(type = "analysis_test_rule")]
impl<'v> StarlarkValue<'v> for FrozenAnalysisTestCallable {
    type Canonical = AnalysisTestCallable<'v>;

    /// Called when this analysis_test_rule callable is invoked in a BUILD file.
    /// Parses the `name` argument and delegates to ANALYSIS_TEST_REGISTER.
    fn invoke(
        &self,
        _me: Value<'v>,
        args: &Arguments<'v, '_>,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> starlark::Result<Value<'v>> {
        // Find `name` in the named arguments
        let named = args.names_map()?;
        let name_str = named
            .iter()
            .find_map(|(k, v)| if k.as_str() == "name" { Some(*v) } else { None })
            .ok_or_else(|| kuro_error::Error::from(AnalysisTestError::MissingName))?
            .unpack_str()
            .ok_or_else(|| kuro_error::Error::from(AnalysisTestError::NameNotString))?;
        if let Ok(register) = ANALYSIS_TEST_REGISTER.get() {
            register(eval, name_str)?;
        }
        Ok(Value::new_none())
    }
}

// ============================================================================
// LinkerInputStub - Stub for linker input
// ============================================================================

/// A stub for LinkerInput used by cc_common.create_linker_input.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct LinkerInputStubGen<V: ValueLifetimeless> {
    owner: V,
    libraries: V,
    user_link_flags: V,
    additional_inputs: V,
}

starlark_complex_value!(pub LinkerInputStub);

impl<V: ValueLifetimeless> Display for LinkerInputStubGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<LinkerInput>")
    }
}

#[starlark::values::starlark_value(type = "LinkerInput")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LinkerInputStubGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "owner" | "libraries" | "user_link_flags" | "additional_inputs"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "owner" => Some(self.owner.to_value()),
            "libraries" => {
                // Return the libraries value (could be a depset or list)
                if self.libraries.to_value().is_none() {
                    Some(heap.alloc(starlark::values::list::AllocList::EMPTY))
                } else {
                    Some(self.libraries.to_value())
                }
            }
            "user_link_flags" => {
                if self.user_link_flags.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.user_link_flags.to_value())
                }
            }
            "additional_inputs" => {
                if self.additional_inputs.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.additional_inputs.to_value())
                }
            }
            _ => None,
        }
    }
}

// ============================================================================
// LinkingContextWithInputs - Linking context with actual linker inputs
// ============================================================================

/// A linking context that stores actual linker inputs.
#[derive(
    Debug,
    ProvidesStaticType,
    NoSerialize,
    Allocative,
    Trace,
    Coerce,
    Freeze
)]
#[repr(C)]
pub struct LinkingContextWithInputsGen<V: ValueLifetimeless> {
    linker_inputs: V,
}

starlark_complex_value!(pub LinkingContextWithInputs);

impl<V: ValueLifetimeless> Display for LinkingContextWithInputsGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<LinkingContext>")
    }
}

#[starlark::values::starlark_value(type = "LinkingContext")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for LinkingContextWithInputsGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(attribute, "linker_inputs")
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "linker_inputs" => {
                if self.linker_inputs.to_value().is_none() {
                    Some(heap.alloc(crate::interpreter::rule_defs::depset::Depset::empty()))
                } else {
                    Some(self.linker_inputs.to_value())
                }
            }
            _ => None,
        }
    }
}
