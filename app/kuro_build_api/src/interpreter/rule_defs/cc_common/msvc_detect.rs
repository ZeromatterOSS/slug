/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! MSVC auto-detection for Windows hosts (via vswhere + SDK layout).

/// Cached MSVC tool paths detected via vswhere.
/// Maps tool name ("cl.exe", "link.exe", "lib.exe") to full path.
static MSVC_TOOL_CACHE: std::sync::OnceLock<Option<MsvcToolPaths>> = std::sync::OnceLock::new();

pub(crate) struct MsvcToolPaths {
    pub(crate) cl: String,
    pub(crate) link: String,
    pub(crate) lib: String,
    /// MSVC standard library include dir (e.g., .../MSVC/14.41/include)
    pub(crate) msvc_include: String,
    /// Windows SDK ucrt include dir
    pub(crate) ucrt_include: String,
    /// Windows SDK um include dir
    pub(crate) um_include: String,
    /// Windows SDK shared include dir
    pub(crate) shared_include: String,
    /// Windows SDK ucrt lib dir
    pub(crate) ucrt_lib: String,
    /// Windows SDK um lib dir
    pub(crate) um_lib: String,
    /// MSVC lib dir
    pub(crate) msvc_lib: String,
}

/// Detect and cache MSVC tool paths on Windows.
/// Convert a path to its Windows 8.3 short form to avoid spaces in args.
/// `_spawnvp` in bootstrap_process_wrapper.cc doesn't quote args with spaces,
/// so we must use short paths to avoid argument splitting.
/// Falls back to the original path if conversion fails.
#[cfg(target_os = "windows")]
fn to_short_path(path: &str) -> String {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    unsafe extern "system" {
        fn GetShortPathNameW(
            lpszLongPath: *const u16,
            lpszShortPath: *mut u16,
            cchBuffer: u32,
        ) -> u32;
    }

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let len = GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0);
        if len == 0 {
            return path.to_string();
        }
        let mut buf = vec![0u16; len as usize];
        let written = GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), len);
        if written == 0 || written >= len {
            return path.to_string();
        }
        String::from_utf16_lossy(&buf[..written as usize])
    }
}

pub(crate) fn get_msvc_tool_paths() -> &'static Option<MsvcToolPaths> {
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
            let vc_tools = PathBuf::from(&vs_path)
                .join("VC")
                .join("Tools")
                .join("MSVC");

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
                cl: to_short_path(&format!("{}\\cl.exe", base)),
                link: to_short_path(&format!("{}\\link.exe", base)),
                lib: to_short_path(&format!("{}\\lib.exe", base)),
                msvc_include: to_short_path(&msvc_include.to_string_lossy()),
                ucrt_include: to_short_path(&ucrt_inc.to_string_lossy()),
                um_include: to_short_path(&um_inc.to_string_lossy()),
                shared_include: to_short_path(&shared_inc.to_string_lossy()),
                ucrt_lib: to_short_path(&ucrt_lib_dir.to_string_lossy()),
                um_lib: to_short_path(&um_lib_dir.to_string_lossy()),
                msvc_lib: to_short_path(&msvc_lib.to_string_lossy()),
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    })
}

/// Returns the list of built-in include directories for the detected MSVC toolchain.
/// On non-Windows, returns an empty list (callers provide Unix defaults).
pub fn get_msvc_include_dirs() -> Vec<String> {
    if let Some(tools) = get_msvc_tool_paths() {
        vec![
            tools.msvc_include.clone(),
            tools.ucrt_include.clone(),
            tools.um_include.clone(),
            tools.shared_include.clone(),
        ]
    } else {
        Vec::new()
    }
}
