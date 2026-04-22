/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! C++ configuration fragment.

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
        let mode = crate::interpreter::rule_defs::build_config::get_compilation_mode();
        Self {
            compilation_mode: mode,
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
    /// Controlled by --strip flag: "always" → true, "never" → false,
    /// "sometimes" (default) → true only for opt compilation mode.
    fn should_strip_binaries(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        let strip = crate::interpreter::rule_defs::build_config::get_strip();
        match strip.as_str() {
            "always" => Ok(true),
            "never" => Ok(false),
            _ => {
                // "sometimes": strip in opt mode only
                Ok(this.compilation_mode == "opt")
            }
        }
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

    /// C/C++ compilation options from --copt flag.
    #[starlark(attribute)]
    fn copts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(crate::interpreter::rule_defs::build_config::get_copts())
    }

    /// C-specific compilation options from --conlyopt flag.
    #[starlark(attribute)]
    fn conlyopts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(crate::interpreter::rule_defs::build_config::get_conlyopts())
    }

    /// C++-specific compilation options from --cxxopt flag.
    #[starlark(attribute)]
    fn cxxopts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(crate::interpreter::rule_defs::build_config::get_cxxopts())
    }

    /// Linker options from --linkopt flag.
    #[starlark(attribute)]
    fn linkopts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(crate::interpreter::rule_defs::build_config::get_linkopts())
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

    /// Whether disable_nocopts is active.
    ///
    /// In Bazel, this controls whether nocopts filtering is disabled.
    /// Always return true for Bazel 9.0+ compatibility (nocopts was removed).
    fn disable_nocopts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Whether C++20 modules support is enabled.
    ///
    /// Returns false since C++20 modules are experimental in Bazel.
    fn experimental_cpp_modules(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether to link static libraries only once.
    ///
    /// Returns true for Bazel 9.0+ behavior where static libraries are
    /// linked only once even when depended on by multiple targets.
    fn experimental_link_static_libraries_once(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Whether shared native deps linking is enabled.
    fn share_native_deps(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether legacy whole archive behavior is removed.
    fn incompatible_remove_legacy_whole_archive(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(true) // Use the modern behavior
    }

    /// Whether to generate .d dependency files for Objective-C.
    fn objc_should_generate_dotd_files(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(true)
    }

    /// Objective-C compilation options.
    #[starlark(attribute)]
    fn objccopts(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<Vec<String>> {
        Ok(Vec::new())
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

    /// Returns the compiler identifier string.
    ///
    /// Used by rules_cc configure_features.bzl for compiler-specific feature setup.
    /// Returns "msvc-cl" on Windows with MSVC, "clang" on macOS, "gcc" on Linux.
    fn compiler(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<String> {
        Ok(detect_compiler())
    }

    /// Returns the target CPU for the current configuration.
    ///
    /// Common values: "k8" (x86_64 Linux), "darwin_x86_64", "darwin_arm64",
    /// "x64_windows", "aarch64".
    fn cpu(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<String> {
        Ok(detect_cpu())
    }

    /// Returns the target CPU (alias for cpu()).
    fn target_cpu(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<String> {
        Ok(detect_cpu())
    }

    /// Returns the host system name.
    fn host_system_name(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<String> {
        Ok(detect_host_system_name())
    }

    /// Returns the target system name.
    fn target_system_name(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<String> {
        Ok(detect_host_system_name())
    }

    /// Returns the target C library identifier.
    fn target_libc(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<String> {
        if cfg!(target_os = "linux") {
            Ok("glibc_2.17".to_owned())
        } else if cfg!(target_os = "macos") {
            Ok("macosx".to_owned())
        } else if cfg!(target_os = "windows") {
            Ok("msvcrt".to_owned())
        } else {
            Ok("local".to_owned())
        }
    }

    /// Returns the ABI version string.
    fn abi_version(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<String> {
        Ok("local".to_owned())
    }

    /// Returns the ABI glibc version string.
    fn abi_glibc_version(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<String> {
        Ok("local".to_owned())
    }

    /// Returns the sysroot path, or None if not set.
    fn sysroot(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        // TODO(fragments): Support --sysroot flag
        Ok(NoneType)
    }

    /// Returns built-in include directories for the compiler.
    fn built_in_include_directories<'v>(
        #[allow(unused_variables)] this: &CppFragment,
        heap: Heap<'v>,
    ) -> starlark::Result<Value<'v>> {
        Ok(heap.alloc(AllocList::EMPTY))
    }

    /// Returns the minimum OS version string, or None if not set.
    fn minimum_os_version(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Returns the FDO path, or None if not set.
    fn fdo_path(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Returns the CS-FDO path, or None if not set.
    fn cs_fdo_path(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Returns the Propeller optimize CC profile path, or None.
    fn propeller_optimize_absolute_cc_profile(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Returns the Propeller optimize LD profile path, or None.
    fn propeller_optimize_absolute_ld_profile(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Whether proto profile is enabled.
    fn proto_profile(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Returns the libc_top label, or None.
    fn libc_top(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Returns the zipper tool label, or None.
    fn zipper(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<NoneType> {
        Ok(NoneType)
    }

    /// Whether stamp binaries is enabled.
    fn stamp_binaries(#[allow(unused_variables)] this: &CppFragment) -> starlark::Result<bool> {
        Ok(false)
    }

    /// Whether interface shared objects are enabled.
    /// In Bazel, this controls whether .ifso (interface shared object) files
    /// are generated for shared libraries.
    fn interface_shared_objects(
        #[allow(unused_variables)] this: &CppFragment,
    ) -> starlark::Result<bool> {
        Ok(false)
    }
}

/// Detect the compiler identifier for the current platform.
fn detect_compiler() -> String {
    if cfg!(target_os = "windows") {
        // On Windows, default to MSVC if available (vswhere detection happens in cc_common)
        "msvc-cl".to_owned()
    } else if cfg!(target_os = "macos") {
        "clang".to_owned()
    } else {
        "gcc".to_owned()
    }
}

/// Detect the target CPU for the current platform.
fn detect_cpu() -> String {
    if cfg!(target_os = "windows") {
        if cfg!(target_arch = "x86_64") {
            "x64_windows".to_owned()
        } else if cfg!(target_arch = "aarch64") {
            "arm64_windows".to_owned()
        } else {
            "x64_windows".to_owned()
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "darwin_arm64".to_owned()
        } else {
            "darwin_x86_64".to_owned()
        }
    } else {
        // Linux
        if cfg!(target_arch = "aarch64") {
            "aarch64".to_owned()
        } else {
            "k8".to_owned()
        }
    }
}

/// Detect the host system name.
fn detect_host_system_name() -> String {
    if cfg!(target_os = "windows") {
        "local".to_owned()
    } else if cfg!(target_os = "macos") {
        "local".to_owned()
    } else {
        "local".to_owned()
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
