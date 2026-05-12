/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Symlink helpers used by bzlmod cell resolution to wire up
//! `bazel-external/<module>` entries without re-downloading sources on each
//! invocation.

use std::path::Path;

/// Compare two symlink-target paths for equality, tolerating the common
/// absolute-vs-relative-to-link-parent mismatch. `std::fs::read_link`
/// returns exactly what was stored in the symlink, and bzlmod resolution
/// sometimes passes absolute paths (registry cache) and sometimes paths
/// relative to the link's parent (project-local). Without this, a symlink
/// created from one flavor and re-checked from the other looks stale and
/// triggers remove+create, which the file watcher picks up as an
/// invalidation on every build.
fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    // Canonicalize only on mismatch — avoids stat'ing on the common path.
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// Ensure a symlink exists from `link` to `target`. Modeled after Bazel's
/// [`FileSystemUtils.ensureSymbolicLink`](https://github.com/bazelbuild/bazel/blob/master/src/main/java/com/google/devtools/build/lib/vfs/FileSystemUtils.java).
///
/// - If symlink already points to target: no-op
/// - If symlink points elsewhere: replace it
/// - If non-symlink exists: return error
pub(crate) fn ensure_symlink(link: &Path, target: &Path) -> std::io::Result<()> {
    if let Ok(existing) = std::fs::read_link(link) {
        if paths_equal(&existing, target) {
            return Ok(());
        }
        // Trace the no-match so the file-watcher Plan 17.4 can chase the
        // residual invalidation events reported in the memory note.
        tracing::debug!(
            "ensure_symlink: replacing stale link {} (was {} -> now {})",
            link.display(),
            existing.display(),
            target.display(),
        );
        if cfg!(windows) {
            let _ = std::fs::remove_dir(link);
            let _ = std::fs::remove_file(link);
        } else {
            std::fs::remove_file(link)?;
        }
    } else if link.exists() {
        tracing::warn!(
            "bazel-external/{} is a real directory, not a symlink - skipping",
            link.file_name().unwrap_or_default().to_string_lossy()
        );
        return Ok(());
    }

    if let Some(parent) = link.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link);

    #[cfg(windows)]
    {
        match std::os::windows::fs::symlink_dir(target, link) {
            Ok(()) => return Ok(()),
            Err(symlink_err) => {
                let output = std::process::Command::new("cmd")
                    .args(["/c", "mklink", "/j"])
                    .arg(link)
                    .arg(target)
                    .output();
                match output {
                    Ok(o) if o.status.success() => return Ok(()),
                    _ => return Err(symlink_err),
                }
            }
        }
    }
}

/// Remove stale symlinks from `external_base_dir` that don't correspond to any
/// resolved module. Handles the case where a module is removed from
/// `MODULE.bazel` or its version changes.
pub(crate) fn cleanup_stale_symlinks(
    external_base_dir: &Path,
    valid_entries: &std::collections::HashSet<String>,
) {
    if !external_base_dir.exists() {
        return;
    }

    let entries = match std::fs::read_dir(external_base_dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!("Could not read bazel-external/ for cleanup: {}", e);
            return;
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !valid_entries.contains(&name) {
            let path = entry.path();
            if path.is_symlink() || (cfg!(windows) && is_junction(&path)) {
                if let Err(e) = if cfg!(windows) {
                    std::fs::remove_dir(&path).or_else(|_| std::fs::remove_file(&path))
                } else {
                    std::fs::remove_file(&path)
                } {
                    tracing::debug!(
                        "Could not remove stale symlink bazel-external/{}: {}",
                        name,
                        e
                    );
                } else {
                    tracing::info!("Removed stale symlink: bazel-external/{}", name);
                }
            }
        }
    }
}

/// Check if a path is a Windows junction point.
#[cfg(windows)]
fn is_junction(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    match std::fs::symlink_metadata(path) {
        Ok(meta) => meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0,
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn is_junction(_path: &Path) -> bool {
    false
}
