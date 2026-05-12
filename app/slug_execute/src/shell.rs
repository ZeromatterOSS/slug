/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Shell detection utilities for cross-platform action execution.

/// Find the appropriate bash executable for the current platform.
///
/// On Unix, this always returns `/bin/bash`.
///
/// On Windows, this resolves the bash path carefully to avoid WSL's bash
/// (`C:\Windows\System32\bash.exe`) which does NOT inherit Windows process
/// environment variables. Instead, it finds MSYS2/Git Bash which properly
/// inherits env vars set via `Command::env()`.
///
/// Resolution order (matches Bazel):
/// 1. `BAZEL_SH` environment variable (explicit override)
/// 2. Git for Windows bash at common installation paths
/// 3. Fall back to `bash.exe` on PATH
pub fn find_bash() -> &'static str {
    #[cfg(not(windows))]
    {
        "/bin/bash"
    }
    #[cfg(windows)]
    {
        use std::sync::OnceLock;
        static BASH_PATH: OnceLock<String> = OnceLock::new();
        BASH_PATH.get_or_init(|| {
            // 1. Check BAZEL_SH env var (Bazel convention)
            if let Ok(bazel_sh) = std::env::var("BAZEL_SH") {
                if std::path::Path::new(&bazel_sh).exists() {
                    return bazel_sh;
                }
            }

            // 2. Check common Git for Windows locations
            let candidates = [
                "C:\\Program Files\\Git\\bin\\bash.exe",
                "C:\\Program Files\\Git\\usr\\bin\\bash.exe",
                "C:\\Program Files (x86)\\Git\\bin\\bash.exe",
            ];
            for path in &candidates {
                if std::path::Path::new(path).exists() {
                    return path.to_string();
                }
            }

            // 3. Fall back to bash.exe on PATH
            "bash.exe".to_owned()
        })
    }
}
