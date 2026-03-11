/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Configuration fragments for Bazel compatibility.
//!
//! In Bazel, `ctx.fragments` provides access to configuration fragments like
//! `ctx.fragments.cpp`, `ctx.fragments.java`, etc. These fragments contain
//! build configuration settings.
//!
//! Reference: thoughts/shared/plans/kuro-bazel-subplans/03-rule-primitives.md

use std::fmt;
use std::fmt::Display;

use allocative::Allocative;
use starlark::environment::Methods;
use starlark::environment::MethodsBuilder;
use starlark::environment::MethodsStatic;
use starlark::starlark_module;
use starlark::starlark_simple_value;
use starlark::values::Heap;
use starlark::values::NoSerialize;
use starlark::values::ProvidesStaticType;
use starlark::values::StarlarkValue;
use starlark::values::Value;
use starlark::values::list::AllocList;
use starlark::values::none::NoneType;
use starlark::values::starlark_value;

// ============================================================================
// CppFragment - C++ configuration fragment
// ============================================================================

/// C++ configuration fragment.
///
/// Accessed via `ctx.fragments.cpp`. Contains C++ build settings like
/// compilation mode, PIC requirements, coverage format, etc.
///
/// Reference: rules_cc uses these in cc/private/toolchain_config/configure_features.bzl
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct CppFragment {
    /// Compilation mode: "opt", "dbg", or "fastbuild"
    compilation_mode: String,
    /// Whether PIC is forced
    force_pic: bool,
    /// Whether to use LLVM coverage map format
    use_llvm_coverage_map_format: bool,
    /// Whether to generate dSYM files on macOS
    apple_generate_dsym: bool,
}

impl Default for CppFragment {
    fn default() -> Self {
        Self {
            compilation_mode: "fastbuild".to_owned(),
            force_pic: false,
            use_llvm_coverage_map_format: false,
            apple_generate_dsym: false,
        }
    }
}

impl Display for CppFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<cpp fragment>")
    }
}

starlark_simple_value!(CppFragment);

#[starlark_value(type = "cpp_fragment")]
impl<'v> StarlarkValue<'v> for CppFragment {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(cpp_fragment_methods)
    }
}

#[starlark_module]
fn cpp_fragment_methods(builder: &mut MethodsBuilder) {
    /// Returns whether -fPIC is forced for compilation.
    fn force_pic(this: &CppFragment) -> starlark::Result<bool> {
        Ok(this.force_pic)
    }

    /// Returns the compilation mode: "opt", "dbg", or "fastbuild".
    fn compilation_mode(this: &CppFragment) -> starlark::Result<String> {
        Ok(this.compilation_mode.clone())
    }

    /// Returns whether to generate .d dependency files.
    fn should_generate_dotd_files(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Returns whether to save temporary files.
    fn save_temps(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Returns whether to process headers in dependencies.
    fn process_headers_in_dependencies(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Returns whether start_end_lib is enabled (for reducing binary size).
    fn start_end_lib(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Returns whether binaries should be stripped.
    fn should_strip_binaries(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Returns whether to use specific tool files (incompatible flag).
    fn incompatible_use_specific_tool_files(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Returns whether to use LLVM coverage map format.
    fn use_llvm_coverage_map_format(this: &CppFragment) -> starlark::Result<bool> {
        Ok(this.use_llvm_coverage_map_format)
    }

    /// Returns the FDO instrumentation path, or None if not set.
    fn fdo_instrument(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        // TODO(fragments): Implement FDO instrumentation path
        Ok(NoneType)
    }

    /// Returns the CS-FDO instrumentation path, or None if not set.
    fn cs_fdo_instrument(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        // TODO(fragments): Implement CS-FDO instrumentation path
        Ok(NoneType)
    }

    /// Whether to generate dSYM files on macOS.
    #[starlark(attribute)]
    fn apple_generate_dsym(this: &CppFragment) -> starlark::Result<bool> {
        Ok(this.apple_generate_dsym)
    }

    /// Whether to generate linkmaps for Objective-C.
    #[starlark(attribute)]
    fn objc_generate_linkmap(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether to strip Objective-C binaries.
    #[starlark(attribute)]
    fn objc_should_strip_binary(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Returns the GRTE top path (GNU Runtime Environment), or None if not set.
    ///
    /// This is used for cross-compilation and specifies the path to the target
    /// system's runtime libraries.
    fn grte_top(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        // TODO(fragments): Implement GRTE top path support
        Ok(NoneType)
    }

    /// Whether to disable host/nonhost feature distinction.
    #[starlark(attribute)]
    fn _dont_enable_host_nonhost(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        // Return true to skip host/nonhost feature enabling
        Ok(true)
    }

    /// Whether per-object debug info is requested.
    #[starlark(attribute)]
    fn fission_active_for_current_compilation_mode(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether to save temporary files.
    #[starlark(attribute)]
    fn save_feature_state(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Custom malloc implementation, or None.
    #[starlark(attribute)]
    fn custom_malloc(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Whether to generate position-independent code.
    #[starlark(attribute)]
    fn copts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// C-specific compilation options.
    #[starlark(attribute)]
    fn conlyopts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// C++-specific compilation options.
    #[starlark(attribute)]
    fn cxxopts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// Linker options.
    #[starlark(attribute)]
    fn linkopts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(Vec::new())
    }

    /// FDO prefetch hints label, or None.
    #[starlark(attribute)]
    fn _fdo_prefetch_hints_label(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Propeller optimize label, or None.
    #[starlark(attribute)]
    fn _propeller_optimize_label(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Memprof profile label, or None.
    #[starlark(attribute)]
    fn _memprof_profile_label(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// FDO optimize label, or None.
    #[starlark(attribute)]
    fn _fdo_optimize_label(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// CS-FDO profile label, or None.
    #[starlark(attribute)]
    fn _cs_fdo_profile_label(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// FDO profile label, or None.
    #[starlark(attribute)]
    fn _fdo_profile_label(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// XFDO profile label, or None.
    #[starlark(attribute)]
    fn _xfdo_profile_label(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Whether FDO is active.
    #[starlark(attribute)]
    fn is_fdo_optimization(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether to use minimal debug info.
    #[starlark(attribute)]
    fn _use_minimal_debug_info(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether to use only fission.
    #[starlark(attribute)]
    fn _use_only_fission(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether experimental CC implementation deps is enabled.
    /// This controls visibility of implementation-only dependencies.
    fn experimental_cc_implementation_deps(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether legacy whole archive behavior is removed.
    fn incompatible_remove_legacy_whole_archive(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(true) // Use the modern behavior
    }

    /// Returns the dynamic linking mode: "FULLY", "OFF", or "DEFAULT".
    ///
    /// - "FULLY": Use fully dynamic linking (shared libraries)
    /// - "OFF": Use static linking only
    /// - "DEFAULT": Use default behavior (let linkstatic attribute decide)
    fn dynamic_mode(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<String> {
        Ok("DEFAULT".to_owned())
    }

    /// Returns strip options to pass to the strip command.
    fn strip_opts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        // Default: strip all symbols
        Ok(vec!["-S".to_owned(), "-p".to_owned()])
    }

    /// Returns the C++ standard to use (e.g., "c++17", "c++20").
    fn cxx_standard(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<String> {
        Ok("c++17".to_owned())
    }

    /// Returns whether to use fission (split DWARF).
    fn use_fission(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Returns whether to generate LLVM LCOV coverage format.
    fn generate_llvm_lcov(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Returns whether to output assembly when compiling.
    fn output_assembly(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }
}

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

// ============================================================================
// ProtoFragment - Protocol Buffers configuration fragment
// ============================================================================

/// Proto configuration fragment stub.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ProtoFragment;

impl Display for ProtoFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "proto fragment")
    }
}

starlark_simple_value!(ProtoFragment);

#[starlark_value(type = "proto_fragment")]
impl<'v> StarlarkValue<'v> for ProtoFragment {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "experimental_protoc_opts"
                | "cc_proto_library_source_suffixes"
                | "cc_proto_library_header_suffixes"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "experimental_protoc_opts" => Some(heap.alloc(AllocList::EMPTY)),
            "cc_proto_library_source_suffixes" => Some(heap.alloc(vec![".pb.cc"])),
            "cc_proto_library_header_suffixes" => Some(heap.alloc(vec![".pb.h"])),
            _ => None,
        }
    }
}

// ============================================================================
// PlatformFragment - Platform configuration fragment
// ============================================================================

/// Platform configuration fragment.
///
/// Accessed via `ctx.fragments.platform`. Contains platform-related settings.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct PlatformFragment;

impl Display for PlatformFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<platform fragment>")
    }
}

starlark_simple_value!(PlatformFragment);

#[starlark_value(type = "platform_fragment")]
impl<'v> StarlarkValue<'v> for PlatformFragment {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(platform_fragment_methods)
    }
}

#[starlark_module]
fn platform_fragment_methods(builder: &mut MethodsBuilder) {
    /// Returns the host platform label.
    #[starlark(attribute)]
    fn host_platform(#[allow(unused_variables)] this: &PlatformFragment) -> starlark::Result<String> {
        Ok("@local_config_platform//:host".to_owned())
    }
}

// ============================================================================
// JavaFragment - Java configuration fragment
// ============================================================================

/// Java configuration fragment stub.
///
/// Accessed via `ctx.fragments.java`. Returns safe defaults for any accessed attribute.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct JavaFragment;

impl Display for JavaFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<java fragment>")
    }
}

starlark_simple_value!(JavaFragment);

#[starlark_value(type = "java_fragment")]
impl<'v> StarlarkValue<'v> for JavaFragment {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "default_javac_flags" | "default_jvm_opts" | "source_version" | "target_version"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "default_javac_flags" | "default_jvm_opts" => Some(heap.alloc(AllocList::EMPTY)),
            "source_version" | "target_version" => Some(heap.alloc("11")),
            _ => None,
        }
    }
}

// ============================================================================
// AppleFragment - Apple configuration fragment
// ============================================================================

/// Apple configuration fragment stub.
///
/// Accessed via `ctx.fragments.apple`. Returns safe defaults for Apple platform settings.
#[derive(Debug, Clone, ProvidesStaticType, NoSerialize, Allocative)]
pub struct AppleFragment;

impl Display for AppleFragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<apple fragment>")
    }
}

starlark_simple_value!(AppleFragment);

#[starlark_value(type = "apple_fragment")]
impl<'v> StarlarkValue<'v> for AppleFragment {
    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "single_arch_platform"
                | "single_arch_cpu"
                | "bitcode_mode"
                | "mandatory_minimum_version"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "single_arch_platform" | "bitcode_mode" => Some(heap.alloc(NoneType)),
            "single_arch_cpu" => {
                if cfg!(target_arch = "aarch64") {
                    Some(heap.alloc("arm64"))
                } else {
                    Some(heap.alloc("x86_64"))
                }
            }
            "mandatory_minimum_version" => Some(heap.alloc(false)),
            _ => None,
        }
    }
}

// ============================================================================
// ConfigurationFragments - Container for all fragments
// ============================================================================

/// Container for configuration fragments.
///
/// Accessed via `ctx.fragments`. Provides access to language-specific
/// configuration fragments like `cpp`, `java`, `apple`, etc.
#[derive(Debug, ProvidesStaticType, NoSerialize, Allocative)]
pub struct ConfigurationFragments {
    cpp: CppFragment,
}

impl Default for ConfigurationFragments {
    fn default() -> Self {
        Self {
            cpp: CppFragment::default(),
        }
    }
}

impl Display for ConfigurationFragments {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<ctx.fragments>")
    }
}

starlark_simple_value!(ConfigurationFragments);

#[starlark_value(type = "configuration_fragments")]
impl<'v> StarlarkValue<'v> for ConfigurationFragments {
    fn get_methods() -> Option<&'static Methods> {
        static RES: MethodsStatic = MethodsStatic::new();
        RES.methods(configuration_fragments_methods)
    }

    fn has_attr(&self, attribute: &str, _heap: Heap<'v>) -> bool {
        matches!(
            attribute,
            "cpp" | "py" | "bazel_py" | "java" | "apple" | "platform" | "proto"
        )
    }

    fn get_attr(&self, attribute: &str, heap: Heap<'v>) -> Option<Value<'v>> {
        match attribute {
            "cpp" => Some(heap.alloc(self.cpp.clone())),
            "py" => Some(heap.alloc(PyFragment)),
            "bazel_py" => Some(heap.alloc(BazelPyFragment)),
            "proto" => Some(heap.alloc(ProtoFragment)),
            "java" => Some(heap.alloc(JavaFragment)),
            "apple" => Some(heap.alloc(AppleFragment)),
            "platform" => Some(heap.alloc(PlatformFragment)),
            _ => None,
        }
    }

    fn dir_attr(&self) -> Vec<String> {
        vec![
            "cpp".to_owned(),
            "py".to_owned(),
            "bazel_py".to_owned(),
            "java".to_owned(),
            "apple".to_owned(),
            "platform".to_owned(),
            "proto".to_owned(),
        ]
    }
}

#[starlark_module]
fn configuration_fragments_methods(builder: &mut MethodsBuilder) {
    /// C++ configuration fragment.
    #[starlark(attribute)]
    fn cpp<'v>(this: &ConfigurationFragments, heap: Heap<'v>) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(this.cpp.clone()))
    }
}

impl CppFragment {
    /// Create a new CppFragment with the specified settings.
    pub fn new(
        compilation_mode: String,
        force_pic: bool,
        use_llvm_coverage_map_format: bool,
        apple_generate_dsym: bool,
    ) -> Self {
        Self {
            compilation_mode,
            force_pic,
            use_llvm_coverage_map_format,
            apple_generate_dsym,
        }
    }
}

impl Clone for CppFragment {
    fn clone(&self) -> Self {
        Self {
            compilation_mode: self.compilation_mode.clone(),
            force_pic: self.force_pic,
            use_llvm_coverage_map_format: self.use_llvm_coverage_map_format,
            apple_generate_dsym: self.apple_generate_dsym,
        }
    }
}

impl ConfigurationFragments {
    /// Create new configuration fragments with the given cpp fragment.
    pub fn new(cpp: CppFragment) -> Self {
        Self { cpp }
    }
}

impl Clone for ConfigurationFragments {
    fn clone(&self) -> Self {
        Self {
            cpp: self.cpp.clone(),
        }
    }
}
