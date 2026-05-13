/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Host platform detection and compiler path helpers shared across cc_common.

use crate::interpreter::rule_defs::cc_common::msvc_detect::get_msvc_tool_paths;

/// Detect whether the compiler is MSVC (cl.exe) based on the compiler path.
pub(crate) fn is_msvc_compiler(compiler_path: &str) -> bool {
    let lower = compiler_path.to_lowercase();
    lower == "cl.exe" || lower == "cl" || lower.ends_with("\\cl.exe") || lower.ends_with("/cl.exe")
}

/// Returns true if the host OS is Windows.
pub(crate) fn is_windows_host() -> bool {
    std::env::consts::OS == "windows"
}

/// Normalize action names from rules_cc: both underscore (cpp_link_dynamic_library)
/// and hyphen (c++-link-dynamic-library) variants are used. Convert to hyphen form
/// for consistent matching.
pub(crate) fn normalize_action_name(name: &str) -> String {
    name.replace("cpp_", "c++-")
        .replace("_link_", "-link-")
        .replace("_compile", "-compile")
        .replace("_dynamic_library", "-dynamic-library")
        .replace("_static_library", "-static-library")
        .replace("_nodeps_", "-nodeps-")
        .replace("_executable", "-executable")
}

/// Resolve a Windows compiler path. If bare "cl.exe", try to find the full MSVC path.
pub(crate) fn resolve_windows_compiler(bare_path: &str) -> String {
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
pub(crate) fn include_flag_for_dir_impl(dir: &str, msvc: bool) -> String {
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

/// Choose an include flag for an explicit `CcCompilationContext` field.
///
/// Unlike source-derived fallback include dirs, these fields already carry
/// Bazel's include class. Preserve that class and ordering instead of applying
/// the external-subdir `-idirafter` heuristic.
pub(crate) fn include_flag_for_context_attr(attr_name: &str, dir: &str, msvc: bool) -> String {
    if msvc {
        return format!("/I{}", dir);
    }
    match attr_name {
        "quote_includes" => format!("-iquote{}", dir),
        "system_includes" | "external_includes" => format!("-isystem{}", dir),
        _ => format!("-I{}", dir),
    }
}

/// Normalize a `buck-out/v2/external_cells/bzlmod/<name>/<version>/...` path to
/// the equivalent `external/<name>/...` path for include path computation.
///
/// This is needed because source artifacts from external bzlmod cells use the
/// full `buck-out/v2/external_cells/` path, but `external/<name>` is a symlink
/// to the same location and is the canonical form for include paths.
pub(crate) fn normalize_external_cells_path(path: &str) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_include_flags_preserve_explicit_include_kind() {
        assert_eq!(
            include_flag_for_context_attr("includes", "external/musl/src/include", false),
            "-Iexternal/musl/src/include"
        );
        assert_eq!(
            include_flag_for_context_attr("system_includes", "external/musl/include", false),
            "-isystemexternal/musl/include"
        );
        assert_eq!(
            include_flag_for_context_attr("quote_includes", "external/musl/src/internal", false),
            "-iquoteexternal/musl/src/internal"
        );
    }

    #[test]
    fn source_derived_deep_external_includes_still_use_idirafter() {
        assert_eq!(
            include_flag_for_dir_impl("external/absl/base/internal", false),
            "-idirafterexternal/absl/base/internal"
        );
    }
}
