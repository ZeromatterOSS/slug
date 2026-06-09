/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Repository rule executor for built-in repository rules.
//!
//! This module implements the actual execution logic for common repository rules
//! like `http_archive`, `git_repository`, and `local_repository`. Rather than
//! invoking Starlark, we implement these natively for better performance and
//! integration with the existing download infrastructure.
//!
//! ## Supported Rules
//!
//! - `http_archive` - Download and extract archive from URL(s)
//! - `git_repository` - Clone a git repository at a specific commit
//! - `local_repository` - Symlink to a local directory
//! - `new_local_repository` - Create a repository from a local directory with custom BUILD

use std::collections::BTreeMap;
use std::io::Cursor;
use std::io::ErrorKind;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use base64::Engine;
use flate2::read::GzDecoder;
use sha2::Digest;
use sha2::Sha256;
use tar::Archive;
use zip::ZipArchive;

use crate::dice_graph::BzlmodCellGraphValue;
use crate::dice_graph::BzlmodEventKind;
use crate::dice_graph::record_bzlmod_event;
use crate::repository_execution::InvocationAttrs;
use crate::repository_execution::REPO_RECORDED_INPUTS_FILE;
use crate::repository_execution::RepositoryExecutionError;
use crate::repository_execution::RepositoryRuleResult;
use crate::repository_execution::write_repository_recorded_inputs;
use crate::repository_invocations::RepositoryInvocation;
use crate::lockfile::compute_sha256_hex;

#[derive(Default)]
struct NativeRepositoryRecordedInputs {
    inputs: Vec<String>,
}

impl NativeRepositoryRecordedInputs {
    fn record_file(
        &mut self,
        invocation: &RepositoryInvocation,
        path: &Path,
    ) -> slug_error::Result<()> {
        let input = crate::lockfile::recorded_file_input(path).map_err(|e| {
            RepositoryExecutionError::ExecutionFailed {
                name: invocation.name.clone(),
                reason: format!(
                    "Failed to record repository input '{}': {}",
                    path.display(),
                    e
                ),
            }
        })?;
        self.inputs.push(input);
        Ok(())
    }

    fn record_unpinned_file_urls(
        &mut self,
        invocation: &RepositoryInvocation,
        urls: &[String],
        sha256: Option<&str>,
        integrity: Option<&str>,
    ) -> slug_error::Result<()> {
        for path in
            crate::unpinned_local_file_url_paths(urls.iter().map(String::as_str), sha256, integrity)
        {
            self.record_file(invocation, &path)?;
        }
        Ok(())
    }
}

/// Global counter for generating unique staging directory names.
static STAGING_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Per-canonical-name lock for repository materialization.
///
/// When two DICE computations with the same `canonical_name` but different
/// keys (e.g. different spec_hash) run concurrently, they would both write
/// to the same `bazel-external/{name}` path and race on disk. This lock
/// serializes materializations of the same canonical name within one daemon.
///
/// Uses `tokio::sync::Mutex` so the guard is `Send` and can be held across
/// `.await` points in the Starlark execution path.
///
/// Cross-daemon concurrency on the same output base is out of scope here.
struct MaterializationLocks {
    locks: std::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl MaterializationLocks {
    fn acquire(&self, canonical_name: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut map = self.locks.lock().unwrap();
        map.entry(canonical_name.to_owned())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }
}

static MATERIALIZATION_LOCKS: LazyLock<MaterializationLocks> =
    LazyLock::new(|| MaterializationLocks {
        locks: std::sync::Mutex::new(std::collections::HashMap::new()),
    });

/// Execute a repository rule invocation using legacy marker reuse.
///
/// Production extension repository execution should call
/// `execute_repository_rule_fresh` after its DICE-owned manifest has decided
/// reuse is invalid. This shortcut remains only to prove that fresh execution
/// does not trust a stale marker.
#[cfg(test)]
pub fn execute_repository_rule(
    invocation: &RepositoryInvocation,
    project_root: &Path,
) -> slug_error::Result<RepositoryRuleResult> {
    let label_resolution = RepositoryLabelResolution::default();
    execute_repository_rule_impl(invocation, project_root, true, &label_resolution)
}

/// Execute a repository rule after the caller has already classified
/// materialization reuse through its own manifest/input state.
#[cfg(test)]
fn execute_repository_rule_fresh(
    invocation: &RepositoryInvocation,
    project_root: &Path,
) -> slug_error::Result<RepositoryRuleResult> {
    let label_resolution = RepositoryLabelResolution::default();
    execute_repository_rule_impl(invocation, project_root, false, &label_resolution)
}

/// Execute a repository rule after the caller has classified materialization
/// reuse and supplied resolver-owned bzlmod label paths.
pub(crate) fn execute_repository_rule_fresh_with_label_resolution(
    invocation: &RepositoryInvocation,
    project_root: &Path,
    label_resolution: &RepositoryLabelResolution,
) -> slug_error::Result<RepositoryRuleResult> {
    execute_repository_rule_impl(invocation, project_root, false, label_resolution)
}

fn execute_repository_rule_impl(
    invocation: &RepositoryInvocation,
    project_root: &Path,
    allow_marker_reuse: bool,
    label_resolution: &RepositoryLabelResolution,
) -> slug_error::Result<RepositoryRuleResult> {
    let attrs = InvocationAttrs::new(invocation);
    let working_dir = project_root.join("bazel-external").join(&invocation.name);

    tracing::info!(
        "Executing repository rule '{}' for '{}' at {:?}",
        invocation.rule_name,
        invocation.name,
        working_dir
    );

    #[cfg(not(test))]
    let _ = allow_marker_reuse;

    // Legacy marker reuse remains test-only. Production callers enter this
    // executor only after RepoMaterializationManifestKey has rejected reuse.
    #[cfg(test)]
    if allow_marker_reuse {
        if is_repo_complete(&working_dir)
            && repo_layout_is_valid_for_invocation(invocation, &working_dir)
        {
            record_bzlmod_event(
                BzlmodEventKind::RepoMaterializationHit,
                invocation.name.as_str(),
            );
            tracing::debug!("Repository '{}' already materialized", invocation.name);
            return Ok(RepositoryRuleResult::success(
                invocation.name.clone(),
                working_dir,
            ));
        }
        let miss_reason = if is_repo_complete(&working_dir) {
            "marker_layout_invalid"
        } else {
            "marker_absent"
        };
        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationMissReason,
            format!("{}:{miss_reason}", invocation.name),
        );
    }

    // Acquire per-canonical-name lock to serialize concurrent materializations
    // of the same output path.
    let _mat_lock_arc = acquire_materialization_lock(&invocation.name);
    // Use blocking_lock since this is a synchronous code path
    let _mat_lock = _mat_lock_arc.blocking_lock();
    let staging_dir = prepare_staging_dir(&working_dir)?;

    let mut recorded_inputs = NativeRepositoryRecordedInputs::default();

    // Dispatch based on rule name - write into staging dir
    let result = match invocation.rule_name.as_str() {
        "http_archive" => execute_http_archive(
            invocation,
            &attrs,
            &staging_dir,
            label_resolution,
            &mut recorded_inputs,
        ),
        "http_file" => execute_http_file(invocation, &attrs, &staging_dir, &mut recorded_inputs),
        "http_jar" => execute_http_jar(invocation, &attrs, &staging_dir, &mut recorded_inputs),
        "git_repository" => execute_git_repository(invocation, &attrs, &staging_dir),
        "local_repository" | "new_local_repository" => {
            execute_local_repository(invocation, &attrs, &staging_dir)
        }
        rule_name => Err(RepositoryExecutionError::NoImplementation {
            name: rule_name.to_owned(),
        }
        .into()),
    };

    match result {
        Ok(()) => {
            write_repository_recorded_inputs(&staging_dir, &recorded_inputs.inputs)?;
            // Write completion marker BEFORE atomic rename so it's part of
            // the atomic swap (the marker moves with the directory).
            mark_repo_complete(&staging_dir)?;
            // Atomically swap staging dir into the canonical path.
            // On success, the canonical path has a complete tree with a valid marker.
            // On failure (first rename fails with non-ENOTEMPTY error), we clean up
            // the staging dir and leave the old canonical path untouched.
            // Note: in the ENOTEMPTY recovery path, the old canonical dir is removed
            // first; if the retry rename then fails, both directories are lost.
            if let Err(e) = finalize_staging_dir(&staging_dir, &working_dir) {
                cleanup_staging_dir(&staging_dir);
                return Err(e);
            }
            Ok(RepositoryRuleResult::success(
                invocation.name.clone(),
                working_dir,
            ))
        }
        Err(e) => {
            // Clean up staging dir on failure, leave canonical path untouched
            cleanup_staging_dir(&staging_dir);
            Err(e)
        }
    }
}

/// Check if a repository is already materialized.
#[cfg(test)]
fn is_repo_complete(working_dir: &Path) -> bool {
    let marker_path = working_dir.join(".slug_repo_complete");
    if !marker_path.exists() {
        return false;
    }
    let Ok(marker) = std::fs::read_to_string(marker_path) else {
        return false;
    };
    let marker = marker.trim();
    if marker == "complete" {
        return true;
    }
    let Some((_, expected_output_state)) = marker.split_once(":output:") else {
        return true;
    };
    repository_output_digest(working_dir).is_ok_and(|digest| digest == expected_output_state)
}

#[cfg(test)]
pub(crate) fn repo_layout_is_valid_for_invocation(
    invocation: &RepositoryInvocation,
    working_dir: &Path,
) -> bool {
    match invocation.rule_name.as_str() {
        "git_repository" | "new_git_repository" => working_dir.join(".git").exists(),
        "local_repository" => local_repository_layout_is_valid(invocation, working_dir),
        "new_local_repository" => new_local_repository_layout_is_valid(invocation, working_dir),
        "_llvm_subproject_repository" => llvm_subproject_layout_is_valid(invocation, working_dir),
        _ => true,
    }
}

#[cfg(test)]
fn local_repository_layout_is_valid(invocation: &RepositoryInvocation, working_dir: &Path) -> bool {
    let Some(source_dir) = local_repository_source_path(invocation, working_dir) else {
        return true;
    };
    local_repository_root_matches_source(&source_dir, working_dir)
}

#[cfg(test)]
fn new_local_repository_layout_is_valid(
    invocation: &RepositoryInvocation,
    working_dir: &Path,
) -> bool {
    if !new_local_repository_build_file_is_valid(invocation, working_dir) {
        return false;
    }
    let Some(source_dir) = local_repository_source_path(invocation, working_dir) else {
        return true;
    };
    let Ok(entries) = std::fs::read_dir(source_dir) else {
        return false;
    };

    let mut checked_any = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if should_skip_local_repository_entry(name_str.as_ref()) {
            continue;
        }
        checked_any = true;
        if !local_repository_entry_matches_source(&entry.path(), &working_dir.join(&name)) {
            return false;
        }
    }

    checked_any || working_dir.join("BUILD.bazel").exists() || working_dir.join("BUILD").exists()
}

#[cfg(unix)]
#[cfg(test)]
fn local_repository_root_matches_source(source_dir: &Path, working_dir: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(working_dir) else {
        return false;
    };
    if !metadata.file_type().is_symlink() {
        return false;
    }

    let Ok(target) = std::fs::read_link(working_dir) else {
        return false;
    };
    let actual = if target.is_absolute() {
        target
    } else {
        working_dir
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(target)
    };

    let expected = source_dir
        .canonicalize()
        .unwrap_or_else(|_| source_dir.to_path_buf());
    let actual = actual.canonicalize().unwrap_or(actual);
    actual == expected
}

#[cfg(not(unix))]
#[cfg(test)]
fn local_repository_root_matches_source(_source_dir: &Path, working_dir: &Path) -> bool {
    working_dir.exists()
}

#[cfg(test)]
fn new_local_repository_build_file_is_valid(
    invocation: &RepositoryInvocation,
    working_dir: &Path,
) -> bool {
    let Some(expected) = invocation
        .attrs
        .get("build_file_content")
        .and_then(|attr| attr.as_string())
    else {
        return true;
    };

    ["BUILD.bazel", "BUILD"].into_iter().any(|name| {
        std::fs::read_to_string(working_dir.join(name))
            .ok()
            .is_some_and(|actual| actual == expected)
    })
}

#[cfg(unix)]
#[cfg(test)]
fn local_repository_entry_matches_source(source_entry: &Path, materialized_entry: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(materialized_entry) else {
        return false;
    };
    if !metadata.file_type().is_symlink() {
        return false;
    }

    let Ok(target) = std::fs::read_link(materialized_entry) else {
        return false;
    };
    let actual = if target.is_absolute() {
        target
    } else {
        materialized_entry
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(target)
    };

    let expected = source_entry
        .canonicalize()
        .unwrap_or_else(|_| source_entry.to_path_buf());
    let actual = actual.canonicalize().unwrap_or(actual);
    actual == expected
}

#[cfg(not(unix))]
#[cfg(test)]
fn local_repository_entry_matches_source(_source_entry: &Path, materialized_entry: &Path) -> bool {
    materialized_entry.exists()
}

pub(crate) fn local_repository_source_path(
    invocation: &RepositoryInvocation,
    working_dir: &Path,
) -> Option<PathBuf> {
    let path = invocation.attrs.get("path")?.as_string()?;
    if path.is_empty() {
        return None;
    }

    let path = Path::new(path);
    let resolved = if path.is_relative() {
        let project_root = working_dir.parent()?.parent()?;
        project_root.join(path)
    } else {
        path.to_path_buf()
    };
    Some(resolved.canonicalize().unwrap_or(resolved))
}

pub(crate) fn should_skip_local_repository_entry(name: &str) -> bool {
    matches!(
        name,
        "BUILD"
            | "BUILD.bazel"
            | "WORKSPACE"
            | "WORKSPACE.bazel"
            | "bazel-external"
            | "bazel-out"
            | "bazel-bin"
            | "bazel-testlogs"
            | "buck-out"
            | ".slug_repo_complete"
    )
}

#[cfg(test)]
fn llvm_subproject_layout_is_valid(invocation: &RepositoryInvocation, working_dir: &Path) -> bool {
    let Some(dir) = invocation
        .attrs
        .get("dir")
        .and_then(|attr| attr.as_string())
    else {
        return true;
    };
    if dir.is_empty() {
        return true;
    }
    let Some(project_root) = working_dir.parent().and_then(|external| external.parent()) else {
        return false;
    };
    let Some(prefix) = invocation.name.rsplit_once('+').map(|(prefix, _)| prefix) else {
        return true;
    };
    let source_dir = project_root
        .join("bazel-external")
        .join(format!("{prefix}+llvm-raw"))
        .join(dir);
    let Ok(entries) = std::fs::read_dir(&source_dir) else {
        return false;
    };

    let mut checked_any = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if should_skip_local_repository_entry(name_str.as_ref()) {
            continue;
        }
        checked_any = true;
        if !local_repository_entry_matches_source(&entry.path(), &working_dir.join(&name)) {
            return false;
        }
    }

    checked_any && working_dir.join("BUILD.bazel").exists()
}

/// Mark a repository as complete.
fn mark_repo_complete(working_dir: &Path) -> slug_error::Result<()> {
    let output_digest = repository_output_digest(working_dir)?;
    std::fs::write(
        working_dir.join(".slug_repo_complete"),
        format!("complete:output:{output_digest}"),
    )
    .map_err(|e| RepositoryExecutionError::WorkingDirFailed {
        reason: format!("Failed to write completion marker: {}", e),
    })?;
    Ok(())
}

pub fn repository_output_digest(working_dir: &Path) -> slug_error::Result<String> {
    let mut hasher = Sha256::new();
    hash_repository_entry(working_dir, Path::new(""), &mut hasher)?;
    let hash = hasher.finalize();
    Ok(format!(
        "sha256-{}",
        base64::engine::general_purpose::STANDARD.encode(hash)
    ))
}

fn hash_repository_entry(
    path: &Path,
    relative: &Path,
    hasher: &mut Sha256,
) -> slug_error::Result<()> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "repository_output_digest".to_owned(),
            reason: format!("Failed to stat '{}': {}", path.display(), e),
        })?;
    hash_path_bytes(relative, hasher);
    if metadata.file_type().is_symlink() {
        hasher.update(b"L");
        let target =
            std::fs::read_link(path).map_err(|e| RepositoryExecutionError::ExecutionFailed {
                name: "repository_output_digest".to_owned(),
                reason: format!("Failed to read symlink '{}': {}", path.display(), e),
            })?;
        hash_path_bytes(&target, hasher);
        return Ok(());
    }
    if metadata.is_dir() {
        hasher.update(b"D");
        let mut entries = std::fs::read_dir(path)
            .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                name: "repository_output_digest".to_owned(),
                reason: format!("Failed to read directory '{}': {}", path.display(), e),
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                name: "repository_output_digest".to_owned(),
                reason: format!("Failed to read directory entry '{}': {}", path.display(), e),
            })?;
        entries.retain(|entry| {
            let file_name = entry.file_name();
            file_name != ".slug_repo_complete" && file_name != REPO_RECORDED_INPUTS_FILE
        });
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            hash_repository_entry(&entry.path(), &relative.join(entry.file_name()), hasher)?;
        }
        return Ok(());
    }
    if metadata.is_file() {
        hasher.update(b"F");
        hasher.update(metadata.len().to_le_bytes());
        let mut file =
            std::fs::File::open(path).map_err(|e| RepositoryExecutionError::ExecutionFailed {
                name: "repository_output_digest".to_owned(),
                reason: format!("Failed to open file '{}': {}", path.display(), e),
            })?;
        let mut buf = [0; 8192];
        loop {
            let n = file
                .read(&mut buf)
                .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                    name: "repository_output_digest".to_owned(),
                    reason: format!("Failed to read file '{}': {}", path.display(), e),
                })?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        return Ok(());
    }
    hasher.update(b"S");
    Ok(())
}

fn hash_path_bytes(path: &Path, hasher: &mut Sha256) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hasher.update(path.as_os_str().as_bytes());
    }
    #[cfg(not(unix))]
    {
        hasher.update(path.to_string_lossy().as_bytes());
    }
    hasher.update([0]);
}

/// Prepare a staging directory for atomic materialization.
///
/// Creates a unique temporary directory as a sibling of `canonical_dir` (same
/// filesystem so `rename` is atomic). The caller populates the staging dir,
/// then calls [`finalize_staging_dir`] to atomically swap it into place.
///
/// On any error during materialization, call [`cleanup_staging_dir`] to remove
/// the staging dir without disturbing the existing canonical directory.
pub(crate) fn prepare_staging_dir(canonical_dir: &Path) -> slug_error::Result<PathBuf> {
    let parent = canonical_dir.parent().ok_or_else(|| {
        RepositoryExecutionError::WorkingDirFailed {
            reason: format!(
                "Canonical dir {:?} has no parent for staging",
                canonical_dir
            ),
        }
    })?;

    // Ensure parent exists
    std::fs::create_dir_all(parent).map_err(|e| {
        RepositoryExecutionError::WorkingDirFailed {
            reason: format!("Failed to create parent directory {:?}: {}", parent, e),
        }
    })?;

    let counter = STAGING_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let staging_dir = canonical_dir.with_file_name(format!(
        "{}.staging.{}.{}",
        canonical_dir.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id(),
        counter,
    ));

    // Clean up any leftover staging dir from a previous failed attempt
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }

    std::fs::create_dir_all(&staging_dir).map_err(|e| {
        RepositoryExecutionError::WorkingDirFailed {
            reason: format!("Failed to create staging directory {:?}: {}", staging_dir, e),
        }
    })?;

    Ok(staging_dir)
}

/// Atomically move a staging directory to the canonical path.
///
/// On POSIX, `rename()` atomically replaces an existing **empty** directory
/// or file at the target path. When the target is a non-empty directory,
/// `rename()` fails with ENOTEMPTY (or EEXIST on some systems). In that
/// case we remove the old directory and retry the rename.
///
/// **TOCTOU window**: after `remove_dir_all` and before the retry `rename`,
/// the canonical path does not exist. New path-based lookups will see
/// ENOENT during this window. Processes that already hold open fds to the
/// old directory can still access it (inode remains alive until the last fd
/// closes). This window is acceptable because per-canonical-name
/// materialization locks serialize access within one daemon, and
/// cross-daemon concurrency is documented as out of scope.
///
/// If the rename fails, this function attempts to clean up the staging dir
/// and returns an error. Note: if the old canonical dir was already removed
/// (ENOTEMPTY path) and the retry rename fails, both directories are lost;
/// the caller will need to re-materialize.
pub(crate) fn finalize_staging_dir(staging_dir: &Path, canonical_dir: &Path) -> slug_error::Result<()> {
    // Try atomic rename first. This succeeds when canonical_dir doesn't
    // exist or is an empty directory (atomic swap).
    match std::fs::rename(staging_dir, canonical_dir) {
        Ok(()) => Ok(()),
        Err(e) if is_enotempty_or_eexist(&e) => {
            // ENOTEMPTY/EEXIST: canonical dir exists and is non-empty.
            // Remove it and retry the rename.
            std::fs::remove_dir_all(canonical_dir).map_err(|rm_err| {
                let _ = std::fs::remove_dir_all(staging_dir);
                RepositoryExecutionError::WorkingDirFailed {
                    reason: format!(
                        "Failed to remove old canonical directory {:?}: {} (original rename error: {})",
                        canonical_dir, rm_err, e
                    ),
                }
            })?;
            std::fs::rename(staging_dir, canonical_dir).map_err(|retry_err| {
                let _ = std::fs::remove_dir_all(staging_dir);
                RepositoryExecutionError::WorkingDirFailed {
                    reason: format!(
                        "Failed to rename staging directory {:?} to canonical {:?} after removing old dir: {}",
                        staging_dir, canonical_dir, retry_err
                    ),
                }
            })?;
            Ok(())
        }
        Err(e) => {
            // Try to clean up staging dir on rename failure
            let _ = std::fs::remove_dir_all(staging_dir);
            Err(RepositoryExecutionError::WorkingDirFailed {
                reason: format!(
                    "Failed to rename staging directory {:?} to canonical {:?}: {}",
                    staging_dir, canonical_dir, e
                ),
            }.into())
        }
    }
}

/// Check whether an `io::Error` is ENOTEMPTY or EEXIST, both of which
/// POSIX permits `rename()` to return when the target directory is non-empty.
///
/// Using symbolic constants from `libc` instead of hardcoded errno values
/// ensures portability across platforms (e.g. ENOTEMPTY=39 on Linux but 66
/// on macOS; EEXIST=17 on both).
#[cfg(unix)]
fn is_enotempty_or_eexist(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(libc::ENOTEMPTY) || e.raw_os_error() == Some(libc::EEXIST)
}

#[cfg(not(unix))]
fn is_enotempty_or_eexist(e: &std::io::Error) -> bool {
    // On non-Unix platforms (Windows), rename() has different semantics.
    // Fall back to checking the platform's "directory not empty" equivalent.
    // Windows rename of a directory over an existing directory fails with
    // ERROR_ACCESS_DENIED (5) or ERROR_DIR_NOT_EMPTY (145).
    e.raw_os_error() == Some(145) || e.raw_os_error() == Some(5)
}

/// Clean up a staging directory after a failed materialization.
pub(crate) fn cleanup_staging_dir(staging_dir: &Path) {
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(staging_dir);
    }
}

/// Acquire the per-canonical-name materialization lock.
///
/// This serializes concurrent materializations of the same output path
/// within one daemon, preventing races when two DICE keys target the same
/// `bazel-external/{name}` directory.
pub(crate) fn acquire_materialization_lock(canonical_name: &str) -> Arc<tokio::sync::Mutex<()>> {
    MATERIALIZATION_LOCKS.acquire(canonical_name)
}

/// Prepare the working directory (legacy non-atomic path).
///
/// Prefer [`prepare_staging_dir`] + [`finalize_staging_dir`] for production
/// materialization. This function remains for test-only code paths that
/// don't need crash safety.
#[cfg(test)]
fn prepare_working_dir(working_dir: &Path) -> slug_error::Result<()> {
    // Remove existing directory if present
    if working_dir.exists() {
        std::fs::remove_dir_all(working_dir).map_err(|e| {
            RepositoryExecutionError::WorkingDirFailed {
                reason: format!("Failed to clean existing directory: {}", e),
            }
        })?;
    }

    // Create fresh directory
    std::fs::create_dir_all(working_dir).map_err(|e| {
        RepositoryExecutionError::WorkingDirFailed {
            reason: format!("Failed to create directory: {}", e),
        }
    })?;

    Ok(())
}

/// Execute http_archive repository rule.
fn execute_http_archive(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
    label_resolution: &RepositoryLabelResolution,
    recorded_inputs: &mut NativeRepositoryRecordedInputs,
) -> slug_error::Result<()> {
    // Get URLs - can be `url` (single) or `urls` (list)
    let urls = get_urls(attrs)?;
    if urls.is_empty() {
        return Err(RepositoryExecutionError::MissingAttribute {
            name: invocation.name.clone(),
            attr: "url or urls".to_owned(),
        }
        .into());
    }

    // Get integrity verification
    let sha256 = attrs.get_optional_string("sha256");
    let integrity = attrs.get_optional_string("integrity");
    let canonical_id = attrs.get_optional_string("canonical_id");
    let strip_prefix = attrs.get_optional_string("strip_prefix");
    recorded_inputs.record_unpinned_file_urls(invocation, &urls, sha256, integrity)?;

    // Try each URL until one succeeds
    let mut last_error = None;
    for url in &urls {
        match download_and_extract(
            url,
            working_dir,
            sha256,
            integrity,
            canonical_id,
            strip_prefix,
        ) {
            Ok(()) => {
                // Create BUILD.bazel if build_file or build_file_content is specified
                if let Some(content) = attrs.get_optional_string("build_file_content") {
                    std::fs::write(working_dir.join("BUILD.bazel"), content).map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: invocation.name.clone(),
                            reason: format!("Failed to write BUILD.bazel: {}", e),
                        }
                    })?;
                } else if let Some(build_file) = attrs.get_optional_string("build_file") {
                    // build_file is a label like "@@repo//path:BUILD.foo" or a file path
                    let build_file_path =
                        resolve_build_file_label(build_file, working_dir, label_resolution)?;
                    recorded_inputs.record_file(invocation, Path::new(&build_file_path))?;
                    let content = std::fs::read_to_string(&build_file_path).map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: invocation.name.clone(),
                            reason: format!(
                                "Could not read build_file '{}' for repository '{}' at '{}': {}",
                                build_file, invocation.name, build_file_path, e
                            ),
                        }
                    })?;
                    std::fs::write(working_dir.join("BUILD.bazel"), content).map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: invocation.name.clone(),
                            reason: format!("Failed to write BUILD.bazel from build_file: {}", e),
                        }
                    })?;
                }

                // Apply patches if specified
                apply_patches(
                    invocation,
                    attrs,
                    working_dir,
                    label_resolution,
                    recorded_inputs,
                )?;

                materialize_llvm_multicall_aliases(working_dir);

                // Create WORKSPACE if not present
                if !working_dir.join("WORKSPACE").exists()
                    && !working_dir.join("WORKSPACE.bazel").exists()
                {
                    std::fs::write(
                        working_dir.join("WORKSPACE.bazel"),
                        format!("workspace(name = \"{}\")\n", invocation.name),
                    )
                    .map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: invocation.name.clone(),
                            reason: format!("Failed to write WORKSPACE.bazel: {}", e),
                        }
                    })?;
                }

                return Ok(());
            }
            Err(e) => {
                tracing::warn!("Failed to download from {}: {}", url, e);
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        RepositoryExecutionError::ExecutionFailed {
            name: invocation.name.clone(),
            reason: "All download URLs failed".to_owned(),
        }
        .into()
    }))
}

fn materialize_llvm_multicall_aliases(working_dir: &Path) {
    let Ok(build_file_content) = std::fs::read_to_string(working_dir.join("BUILD.bazel")) else {
        return;
    };
    if !build_file_content.contains("declare_llvm_targets") {
        return;
    }

    let llvm_exe = working_dir.join("bin").join("llvm.exe");
    if !llvm_exe.is_file() {
        return;
    }

    for tool in LLVM_MULTICALL_TOOLS {
        let alias = working_dir.join("bin").join(format!("{tool}.exe"));
        if alias.exists() {
            continue;
        }
        let _ = std::fs::hard_link(&llvm_exe, &alias)
            .or_else(|_| std::fs::copy(&llvm_exe, &alias).map(|_| ()));
    }
}

const LLVM_MULTICALL_TOOLS: &[&str] = &[
    "c++filt",
    "clang",
    "clang++",
    "clang-cl",
    "clang-cpp",
    "clang-scan-deps",
    "dsymutil",
    "gcov",
    "lld",
    "ld.lld",
    "ld64.lld",
    "lld-link",
    "wasm-ld",
    "llvm-addr2line",
    "llvm-ar",
    "llvm-bitcode-strip",
    "llvm-cgdata",
    "llvm-cov",
    "llvm-cxxfilt",
    "llvm-debuginfod-find",
    "llvm-dlltool",
    "llvm-dwp",
    "llvm-gsymutil",
    "llvm-ifs",
    "llvm-install-name-tool",
    "llvm-libtool-darwin",
    "llvm-link",
    "llvm-lipo",
    "llvm-ml",
    "llvm-mt",
    "llvm-nm",
    "llvm-objcopy",
    "llvm-objdump",
    "llvm-otool",
    "llvm-profdata",
    "llvm-ranlib",
    "llvm-rc",
    "llvm-readelf",
    "llvm-readobj",
    "llvm-size",
    "llvm-strip",
    "llvm-symbolizer",
    "llvm-windres",
    "sancov",
];

/// Apply patches to a repository after extraction.
///
/// Supports:
/// - `patches`: list of patch file paths to apply
/// - `patch_args`: arguments to pass to `patch` command (default: ["-p1"])
/// - `patch_cmds`: shell commands to run after patching
fn apply_patches(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
    label_resolution: &RepositoryLabelResolution,
    recorded_inputs: &mut NativeRepositoryRecordedInputs,
) -> slug_error::Result<()> {
    // Apply patch files
    if let Some(patches) = attrs.get_string_list("patches") {
        let default_patch_args = ["-p1".to_owned()];
        let patch_args = attrs
            .get_string_list("patch_args")
            .unwrap_or(&default_patch_args);

        for patch_path in patches {
            tracing::info!(
                "Applying patch '{}' to repository '{}'",
                patch_path,
                invocation.name
            );

            let resolved_patch_path =
                match resolve_build_file_label(patch_path, working_dir, label_resolution) {
                    Ok(path) => path,
                    Err(e) => {
                        tracing::warn!(
                            "Patch '{}' could not be resolved for repository '{}' (non-fatal): {}",
                            patch_path,
                            invocation.name,
                            e
                        );
                        continue;
                    }
                };
            recorded_inputs.record_file(invocation, Path::new(&resolved_patch_path))?;
            if let Err(e) =
                apply_patch_file(Path::new(&resolved_patch_path), patch_args, working_dir)
            {
                tracing::warn!("Patch '{}' failed (non-fatal): {}", patch_path, e);
            }
        }
    }

    // Run patch commands
    if let Some(patch_cmds) = attrs.get_string_list("patch_cmds") {
        for cmd_str in patch_cmds {
            tracing::info!("Running patch_cmd for '{}': {}", invocation.name, cmd_str);

            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let flag = if cfg!(windows) { "/c" } else { "-c" };

            let output = Command::new(shell)
                .arg(flag)
                .arg(cmd_str)
                .current_dir(working_dir)
                .output()
                .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                    name: invocation.name.clone(),
                    reason: format!("Failed to run patch_cmd '{}': {}", cmd_str, e),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::warn!("patch_cmd '{}' failed (non-fatal): {}", cmd_str, stderr);
            }
        }
    }

    Ok(())
}

fn apply_patch_file(
    patch_path: &Path,
    patch_args: &[String],
    working_dir: &Path,
) -> Result<(), String> {
    let mut cmd = Command::new("patch");
    for arg in patch_args {
        cmd.arg(arg);
    }
    match cmd
        .arg("-i")
        .arg(patch_path)
        .current_dir(working_dir)
        .output()
    {
        Ok(output) if output.status.success() => return Ok(()),
        Ok(output) => {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        Err(e) if e.kind() != ErrorKind::NotFound => {
            return Err(format!("Failed to run patch command: {e}"));
        }
        Err(_) => {}
    }

    let strip_arg = git_apply_strip_arg(patch_args);
    let output = Command::new("git")
        .args(["apply", "--unsafe-paths", "--whitespace=nowarn", &strip_arg])
        .arg(patch_path)
        .current_dir(working_dir)
        .output()
        .map_err(|e| format!("Failed to run patch fallback via git apply: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ))
    }
}

fn git_apply_strip_arg(patch_args: &[String]) -> String {
    let mut iter = patch_args.iter();
    while let Some(arg) = iter.next() {
        if let Some(strip) = arg.strip_prefix("-p") {
            if !strip.is_empty() {
                return format!("-p{strip}");
            }
            if let Some(strip) = iter.next() {
                return format!("-p{strip}");
            }
        }
    }
    "-p1".to_owned()
}

/// Execute http_file repository rule.
///
/// Downloads a single file and makes it available as a target.
/// Creates a BUILD.bazel that exposes the file via `filegroup`.
fn execute_http_file(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
    recorded_inputs: &mut NativeRepositoryRecordedInputs,
) -> slug_error::Result<()> {
    let urls = get_urls(attrs)?;
    if urls.is_empty() {
        return Err(RepositoryExecutionError::MissingAttribute {
            name: invocation.name.clone(),
            attr: "url or urls".to_owned(),
        }
        .into());
    }

    let sha256 = attrs.get_optional_string("sha256");
    let integrity = attrs.get_optional_string("integrity");
    let canonical_id = attrs.get_optional_string("canonical_id");
    let downloaded_file_path = attrs
        .get_optional_string("downloaded_file_path")
        .unwrap_or("downloaded");
    recorded_inputs.record_unpinned_file_urls(
        invocation,
        &urls,
        sha256.as_deref(),
        integrity.as_deref(),
    )?;

    // Download the file
    let mut last_error = None;
    let mut data = read_cached_repository_download(
        sha256.as_deref(),
        integrity.as_deref(),
        canonical_id.as_deref(),
    );
    for url in &urls {
        if data.is_some() {
            break;
        }
        match download_url(url) {
            Ok(d) => {
                if let Some(expected) = sha256.as_deref() {
                    verify_sha256(&d, expected)?;
                }
                if let Some(expected) = integrity.as_deref() {
                    verify_integrity(&d, expected)?;
                }
                write_cached_repository_download(
                    sha256.as_deref(),
                    integrity.as_deref(),
                    canonical_id.as_deref(),
                    &d,
                );
                data = Some(d);
                break;
            }
            Err(e) => {
                tracing::warn!("Failed to download from {}: {}", url, e);
                last_error = Some(e);
            }
        }
    }

    let file_data = data.ok_or_else(|| {
        last_error.unwrap_or_else(|| {
            RepositoryExecutionError::ExecutionFailed {
                name: invocation.name.clone(),
                reason: "All download URLs failed".to_owned(),
            }
            .into()
        })
    })?;

    // Verify integrity
    if let Some(expected) = sha256.as_deref() {
        verify_sha256(&file_data, expected)?;
    }
    if let Some(expected) = integrity.as_deref() {
        verify_integrity(&file_data, expected)?;
    }

    // Warn on unpinned downloads
    if sha256.is_none() && integrity.is_none() {
        let computed = compute_sha256_hex(&file_data);
        tracing::warn!(
            "Downloaded {} without integrity verification for http_file '{}'. \
             Add `sha256 = \"{computed}\"` to verify this download.",
            urls.first().map(|s| s.as_str()).unwrap_or("unknown"),
            invocation.name,
        );
    }

    // Write the file. Bazel's http_file places the downloaded file in a
    // "file/" subdirectory so Label("@repo//file:downloaded") resolves correctly.
    let file_dir = working_dir.join("file");
    std::fs::create_dir_all(&file_dir).ok();
    let dest_path = file_dir.join(downloaded_file_path);
    std::fs::write(&dest_path, &file_data).map_err(|e| {
        RepositoryExecutionError::ExecutionFailed {
            name: invocation.name.clone(),
            reason: format!("Failed to write downloaded file: {}", e),
        }
    })?;

    // Set executable if requested
    #[cfg(unix)]
    {
        let executable = attrs.get_bool("executable", false);
        if executable {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o755));
        }
    }

    // Create root BUILD.bazel (empty package)
    std::fs::write(working_dir.join("BUILD.bazel"), "").ok();

    // Create file/BUILD.bazel (Bazel http_file convention)
    let file_build = format!(
        r#"package(default_visibility = ["//visibility:public"])

exports_files(["{}"])
"#,
        downloaded_file_path
    );
    std::fs::write(file_dir.join("BUILD.bazel"), file_build).ok();

    Ok(())
}

/// Execute http_jar repository rule.
///
/// Downloads a JAR file and makes it available as a java_import target.
/// Falls back to filegroup if java rules not available.
fn execute_http_jar(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
    recorded_inputs: &mut NativeRepositoryRecordedInputs,
) -> slug_error::Result<()> {
    let urls = get_urls(attrs)?;
    if urls.is_empty() {
        return Err(RepositoryExecutionError::MissingAttribute {
            name: invocation.name.clone(),
            attr: "url or urls".to_owned(),
        }
        .into());
    }

    let sha256 = attrs.get_optional_string("sha256");
    let integrity = attrs.get_optional_string("integrity");
    let canonical_id = attrs.get_optional_string("canonical_id");
    recorded_inputs.record_unpinned_file_urls(
        invocation,
        &urls,
        sha256.as_deref(),
        integrity.as_deref(),
    )?;

    // Download the jar
    let mut last_error = None;
    let mut data = read_cached_repository_download(
        sha256.as_deref(),
        integrity.as_deref(),
        canonical_id.as_deref(),
    );
    for url in &urls {
        if data.is_some() {
            break;
        }
        match download_url(url) {
            Ok(d) => {
                if let Some(expected) = sha256.as_deref() {
                    verify_sha256(&d, expected)?;
                }
                if let Some(expected) = integrity.as_deref() {
                    verify_integrity(&d, expected)?;
                }
                write_cached_repository_download(
                    sha256.as_deref(),
                    integrity.as_deref(),
                    canonical_id.as_deref(),
                    &d,
                );
                data = Some(d);
                break;
            }
            Err(e) => {
                tracing::warn!("Failed to download from {}: {}", url, e);
                last_error = Some(e);
            }
        }
    }

    let jar_data = data.ok_or_else(|| {
        last_error.unwrap_or_else(|| {
            RepositoryExecutionError::ExecutionFailed {
                name: invocation.name.clone(),
                reason: "All download URLs failed".to_owned(),
            }
            .into()
        })
    })?;

    // Verify integrity
    if let Some(expected) = sha256.as_deref() {
        verify_sha256(&jar_data, expected)?;
    }
    if let Some(expected) = integrity.as_deref() {
        verify_integrity(&jar_data, expected)?;
    }

    // Warn on unpinned downloads
    if sha256.is_none() && integrity.is_none() {
        let computed = compute_sha256_hex(&jar_data);
        tracing::warn!(
            "Downloaded {} without integrity verification for http_jar '{}'. \
             Add `sha256 = \"{computed}\"` to verify this download.",
            urls.first().map(|s| s.as_str()).unwrap_or("unknown"),
            invocation.name,
        );
    }

    // Write the jar file
    let jar_filename = format!("{}.jar", invocation.name);
    std::fs::write(working_dir.join(&jar_filename), &jar_data).map_err(|e| {
        RepositoryExecutionError::ExecutionFailed {
            name: invocation.name.clone(),
            reason: format!("Failed to write jar file: {}", e),
        }
    })?;

    // Create BUILD.bazel with filegroup (since java_import requires rules_java)
    let build_content = format!(
        r#"package(default_visibility = ["//visibility:public"])

filegroup(
    name = "jar",
    srcs = ["{}"],
)

exports_files(["{}"])
"#,
        jar_filename, jar_filename
    );
    std::fs::write(working_dir.join("BUILD.bazel"), build_content).ok();

    Ok(())
}

/// Get URLs from attributes (handles both `url` and `urls`).
fn get_urls(attrs: &InvocationAttrs) -> slug_error::Result<Vec<String>> {
    let mut urls = Vec::new();

    // Check `url` attribute first
    if let Some(url) = attrs.get_optional_string("url") {
        urls.push(url.to_owned());
    }

    // Check `urls` attribute
    if let Some(url_list) = attrs.get_string_list("urls") {
        urls.extend(url_list.iter().cloned());
    }

    Ok(urls)
}

/// Resolve a repository-rule file attribute that may be a Bazel label.
///
/// `build_file` and `patches` are executed while materializing
/// `{project_root}/bazel-external/{repo}`. Semantic parsing goes through
/// `repo_mapping`, and repository paths must come from the resolver-owned
/// bzlmod cell graph supplied by the caller.
fn resolve_build_file_label(
    label: &str,
    working_dir: &Path,
    label_resolution: &RepositoryLabelResolution,
) -> slug_error::Result<String> {
    let Some(parsed) =
        crate::repo_mapping::canonicalize_label_with_package_context(label, "", "", None)
    else {
        return Ok(label.to_owned());
    };

    let project_root = working_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf);

    let Some(project_root) = project_root else {
        return Ok(label_to_relative_fragment(&parsed)
            .to_string_lossy()
            .to_string());
    };

    let repo = repository_executor_repo_dir_name(parsed.repo().as_str());
    if let Some(path) = label_resolution.resolve_label(&project_root, &parsed, &repo) {
        return Ok(path.to_string_lossy().to_string());
    }
    Err(RepositoryExecutionError::ExecutionFailed {
        name: repo,
        reason: format!(
            "label '{}' references a repository that is not present in the resolver-owned bzlmod cell graph",
            label
        ),
    }
    .into())
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Allocative)]
pub(crate) struct RepositoryLabelResolution {
    cell_paths: BTreeMap<String, PathBuf>,
}

impl RepositoryLabelResolution {
    pub(crate) fn from_cell_graph(project_root: &Path, cell_graph: &BzlmodCellGraphValue) -> Self {
        let mut cell_paths = BTreeMap::new();
        if !cell_graph.root_module_name.is_empty() {
            cell_paths.insert(
                cell_graph.root_module_name.to_owned(),
                project_root.to_path_buf(),
            );
        }
        for cell in cell_graph.cells.iter() {
            let path = project_relative_or_absolute(project_root, &cell.path);
            cell_paths.insert(cell.name.clone(), path.clone());
            if let Some(canonical_module_name) = canonical_module_name_from_cell_path(&cell.path) {
                cell_paths.entry(canonical_module_name).or_insert(path);
            }
        }
        for cell in cell_graph.extension_cells.iter() {
            let path = project_relative_or_absolute(project_root, &cell.path);
            cell_paths.insert(cell.canonical_name.clone(), path);
        }

        let mut resolution = Self { cell_paths };
        for alias in cell_graph.root_aliases.iter() {
            resolution.insert_alias(&alias.apparent_name, &alias.target_name);
        }
        for alias in cell_graph.dynamic_aliases.iter() {
            resolution.insert_alias(&alias.apparent_name, &alias.canonical_name);
        }
        // Scoped aliases need the declaring module to resolve correctly. The
        // native repository executor does not yet carry that owner context, so
        // do not flatten them into a process-wide apparent-name map here.
        resolution
    }

    fn resolve_label(
        &self,
        project_root: &Path,
        label: &crate::repo_mapping::CanonicalLabel,
        repo: &str,
    ) -> Option<PathBuf> {
        if repo.is_empty() {
            return Some(label_path_under(project_root, label));
        }

        self.cell_paths
            .get(repo)
            .map(|repo_path| label_path_under(repo_path, label))
    }

    fn insert_alias(&mut self, apparent_name: &str, target_name: &str) {
        let Some(target_path) = self.cell_paths.get(target_name).cloned() else {
            return;
        };
        self.cell_paths
            .entry(apparent_name.to_owned())
            .or_insert(target_path);
    }
}

fn canonical_module_name_from_cell_path(path: &str) -> Option<String> {
    let external_repo = path.strip_prefix("bazel-external/")?.split('/').next()?;
    (external_repo.ends_with('+') && !external_repo.starts_with('+'))
        .then(|| external_repo.to_owned())
}

fn project_relative_or_absolute(project_root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        project_root.join(path)
    }
}

fn repository_executor_repo_dir_name(repo: &str) -> String {
    if repo.starts_with('+') {
        format!("_main{}", repo)
    } else {
        repo.to_owned()
    }
}

fn label_path_under(
    base: impl Into<PathBuf>,
    label: &crate::repo_mapping::CanonicalLabel,
) -> PathBuf {
    let mut path = base.into();
    if !label.package().is_empty() {
        path.push(label.package());
    }
    path.push(label.target());
    path
}

fn label_to_relative_fragment(label: &crate::repo_mapping::CanonicalLabel) -> PathBuf {
    label_path_under(label.repo().as_str(), label)
}

/// Download and extract an archive.
fn download_and_extract(
    url: &str,
    dest_dir: &Path,
    sha256: Option<&str>,
    integrity: Option<&str>,
    canonical_id: Option<&str>,
    strip_prefix: Option<&str>,
) -> slug_error::Result<()> {
    if let Some(data) = read_cached_repository_download(sha256, integrity, canonical_id) {
        extract_archive(&data, dest_dir, strip_prefix)?;
        return Ok(());
    }

    tracing::info!("Downloading from {}", url);

    // Download using curl or wget
    let data = download_url(url)?;

    // Verify integrity
    if let Some(expected) = sha256 {
        verify_sha256(&data, expected)?;
    }
    if let Some(expected) = integrity {
        verify_integrity(&data, expected)?;
    }

    // Warn on unpinned downloads: when neither sha256 nor integrity is
    // provided, the download is unverified and not reproducible. Bazel 9
    // also warns in this case. Surface the computed sha256 so the user can
    // pin it.
    if sha256.is_none() && integrity.is_none() {
        let computed = compute_sha256_hex(&data);
        tracing::warn!(
            "Downloaded {} without integrity verification. \
             Add `sha256 = \"{computed}\"` to verify this download.",
            url,
        );
        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationMissReason,
            &format!("unpinned_download:{url}:sha256={computed}"),
        );
    }

    write_cached_repository_download(sha256, integrity, canonical_id, &data);

    // Extract
    extract_archive(&data, dest_dir, strip_prefix)?;

    Ok(())
}

fn repository_download_cache_key(sha256: Option<&str>, integrity: Option<&str>) -> Option<String> {
    if let Some(integrity) = integrity.filter(|s| !s.is_empty()) {
        Some(integrity.to_owned())
    } else {
        sha256
            .filter(|s| !s.is_empty())
            .map(|sha256| format!("sha256-hex-{sha256}"))
    }
}

fn read_cached_repository_download(
    sha256: Option<&str>,
    integrity: Option<&str>,
    canonical_id: Option<&str>,
) -> Option<Vec<u8>> {
    let key = repository_download_cache_key(sha256, integrity)?;
    let cache = crate::cache::ModuleCache::new().ok()?;
    let data = cache
        .read_download_with_canonical_id(&key, canonical_id.unwrap_or(""))
        .ok()??;
    if let Some(expected) = sha256.filter(|s| !s.is_empty()) {
        if verify_sha256(&data, expected).is_err() {
            tracing::warn!("Ignoring repository download cache entry with mismatched sha256");
            return None;
        }
    }
    if let Some(expected) = integrity.filter(|s| !s.is_empty()) {
        if verify_integrity(&data, expected).is_err() {
            tracing::warn!("Ignoring repository download cache entry with mismatched integrity");
            return None;
        }
    }
    Some(data)
}

fn write_cached_repository_download(
    sha256: Option<&str>,
    integrity: Option<&str>,
    canonical_id: Option<&str>,
    data: &[u8],
) {
    let Some(key) = repository_download_cache_key(sha256, integrity) else {
        return;
    };
    match crate::cache::ModuleCache::new().and_then(|cache| {
        cache.write_download_with_canonical_id(&key, canonical_id.unwrap_or(""), data)
    }) {
        Ok(_) => {}
        Err(e) => tracing::debug!("Failed to write repository download cache entry: {}", e),
    }
}

/// Download a URL using curl or wget.
fn download_url(url: &str) -> slug_error::Result<Vec<u8>> {
    // Timeouts: split connect and total. Stuck TCP connects get ~30s before
    // skipping to the next URL in the caller's fallback list. Stalled
    // in-flight transfers get up to 60s wall-time total. Previously this
    // function allowed `--max-time 300` per URL with no `--connect-timeout`
    // and then tried wget as a fallback on the same URL — a single
    // unreachable mirror (e.g. gmplib.org intermittent outage) blocked the
    // daemon thread for 5+5 minutes before the next URL in the caller's
    // urls[] list was tried. On slow-but-live mirrors, the caller's next
    // URL is typically faster; favour falling through quickly.
    // See Plan 10 Phase 7 diagnostic findings.
    const CONNECT_TIMEOUT_SECS: &str = "30";
    const TOTAL_TIMEOUT_SECS: &str = "60";

    // Try curl first. On Windows, use curl.exe to avoid PowerShell alias.
    let curl_cmd = if cfg!(windows) { "curl.exe" } else { "curl" };
    let output = Command::new(curl_cmd)
        .args([
            "-fsSL",
            "--connect-timeout",
            CONNECT_TIMEOUT_SECS,
            "--max-time",
            TOTAL_TIMEOUT_SECS,
            url,
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => return Ok(output.stdout),
        Ok(output) => {
            // curl ran but the URL failed. Common causes at this point are
            // HTTP errors (4xx/5xx) or timeouts. wget is unlikely to
            // recover from HTTP errors, and if the failure was a timeout,
            // wget will time out on the same URL for the same duration.
            // Skip wget; surface the error and let the caller try the next
            // URL in its fallback list.
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::debug!("curl failed for {}: {}", url, stderr);
            return Err(RepositoryExecutionError::ExecutionFailed {
                name: url.to_owned(),
                reason: format!("Download failed: {}", stderr),
            }
            .into());
        }
        Err(e) => {
            tracing::debug!("curl not available: {}", e);
        }
    }

    // curl not found - try wget as the primary tool.
    let output = Command::new("wget")
        .args([
            "-q",
            "-O",
            "-",
            "--connect-timeout",
            CONNECT_TIMEOUT_SECS,
            "--timeout",
            TOTAL_TIMEOUT_SECS,
            url,
        ])
        .output()
        .map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: url.to_owned(),
            reason: format!("Neither curl nor wget available: {}", e),
        })?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(RepositoryExecutionError::ExecutionFailed {
            name: url.to_owned(),
            reason: format!("Download failed: {}", stderr),
        }
        .into())
    }
}

/// Verify SHA256 hash.
fn verify_sha256(data: &[u8], expected: &str) -> slug_error::Result<()> {
    let hash = Sha256::digest(data);
    let computed = hex::encode(hash);

    if computed.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(RepositoryExecutionError::ExecutionFailed {
            name: "integrity".to_owned(),
            reason: format!("SHA256 mismatch: expected {}, got {}", expected, computed),
        }
        .into())
    }
}

/// Verify SRI integrity hash.
fn verify_integrity(data: &[u8], expected: &str) -> slug_error::Result<()> {
    use base64::Engine;

    let (algo, hash) =
        expected
            .split_once('-')
            .ok_or_else(|| RepositoryExecutionError::ExecutionFailed {
                name: "integrity".to_owned(),
                reason: format!("Invalid integrity format: {}", expected),
            })?;

    if algo != "sha256" {
        return Err(RepositoryExecutionError::ExecutionFailed {
            name: "integrity".to_owned(),
            reason: format!("Unsupported hash algorithm: {}", algo),
        }
        .into());
    }

    let expected_bytes = base64::engine::general_purpose::STANDARD
        .decode(hash)
        .map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "integrity".to_owned(),
            reason: format!("Invalid base64: {}", e),
        })?;

    let computed = Sha256::digest(data);
    if computed.as_slice() == expected_bytes.as_slice() {
        Ok(())
    } else {
        Err(RepositoryExecutionError::ExecutionFailed {
            name: "integrity".to_owned(),
            reason: format!("Integrity mismatch"),
        }
        .into())
    }
}

/// Extract an archive, auto-detecting format.
fn extract_archive(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> slug_error::Result<()> {
    // Try tar.gz first
    if extract_tar_gz(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try tar.xz
    if extract_tar_xz(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try tar.zst
    if extract_tar_zst(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    // Try zip
    if extract_zip(data, dest_dir, strip_prefix).is_ok() {
        return Ok(());
    }

    Err(RepositoryExecutionError::ExecutionFailed {
        name: "extract".to_owned(),
        reason: format!(
            "Unknown archive format ({} bytes, starts with {:02x?})",
            data.len(),
            &data[..data.len().min(8)]
        ),
    }
    .into())
}

/// Ensure a candidate path stays within dest_dir after lexical normalization.
///
/// This rejects absolute paths and any path that, after resolving `.` and `..`
/// components, would escape `dest_dir`. It does NOT call `canonicalize()` because
/// the target may not exist yet and canonicalize follows symlinks.
fn contain_path(dest_dir: &Path, candidate: &Path) -> slug_error::Result<PathBuf> {
    // Reject absolute paths outright
    if candidate.is_absolute() {
        return Err(RepositoryExecutionError::PathTraversal {
            entry: candidate.to_string_lossy().to_string(),
        }
        .into());
    }
    // Lexically normalize by folding components
    let normalized = dest_dir
        .join(candidate)
        .components()
        .fold(PathBuf::new(), |mut acc, c| {
            match c {
                std::path::Component::ParentDir => {
                    acc.pop();
                }
                std::path::Component::Normal(n) => acc.push(n),
                std::path::Component::CurDir => {}
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    acc.push(c.as_os_str());
                }
            }
            acc
        });
    // Check that normalized path starts with dest_dir
    if !normalized.starts_with(dest_dir) {
        return Err(RepositoryExecutionError::PathTraversal {
            entry: candidate.to_string_lossy().to_string(),
        }
        .into());
    }
    Ok(normalized)
}

/// Lexically normalize a path and check that it stays within `parent`.
///
/// Unlike `contain_path`, this accepts already-absolute paths (e.g. from joining
/// a relative symlink target with its parent directory). It normalizes the path
/// lexically and verifies the result starts with `parent`.
fn path_is_within(parent: &Path, candidate: &Path) -> slug_error::Result<PathBuf> {
    let normalized = candidate
        .components()
        .fold(PathBuf::new(), |mut acc, c| {
            match c {
                std::path::Component::ParentDir => {
                    acc.pop();
                }
                std::path::Component::Normal(n) => acc.push(n),
                std::path::Component::CurDir => {}
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    acc.push(c.as_os_str());
                }
            }
            acc
        });
    if !normalized.starts_with(parent) {
        return Err(RepositoryExecutionError::PathTraversal {
            entry: candidate.to_string_lossy().to_string(),
        }
        .into());
    }
    Ok(normalized)
}

/// Extract a tar archive from any reader.
fn extract_tar_from_reader<R: std::io::Read>(
    reader: R,
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> slug_error::Result<()> {
    let mut archive = Archive::new(reader);

    for entry_result in
        archive
            .entries()
            .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                name: "extract".to_owned(),
                reason: e.to_string(),
            })?
    {
        let mut entry = entry_result.map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "extract".to_owned(),
            reason: e.to_string(),
        })?;

        let path = entry
            .path()
            .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                name: "extract".to_owned(),
                reason: e.to_string(),
            })?;

        // Apply strip_prefix and route through containment check
        let dest_path = if let Some(prefix) = strip_prefix {
            let path_str = path.to_string_lossy();
            if let Some(stripped) = path_str.strip_prefix(prefix) {
                let stripped = stripped.trim_start_matches('/');
                if stripped.is_empty() {
                    continue;
                }
                contain_path(dest_dir, Path::new(stripped))?
            } else if path_str.starts_with(prefix.trim_end_matches('/')) {
                let prefix_with_slash = format!("{}/", prefix.trim_end_matches('/'));
                if let Some(stripped) = path_str.strip_prefix(&prefix_with_slash) {
                    if stripped.is_empty() {
                        continue;
                    }
                    contain_path(dest_dir, Path::new(stripped))?
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            contain_path(dest_dir, &*path)?
        };

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to create parent directory: {}", e),
                }
            })?;
        }

        // Extract based on entry type
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to create directory: {}", e),
                }
            })?;
        } else if entry_type.is_file() {
            let mut file = std::fs::File::create(&dest_path).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to create file: {}", e),
                }
            })?;
            std::io::copy(&mut entry, &mut file).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to write file contents: {}", e),
                }
            })?;

            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(mode) = entry.header().mode() {
                    let _ =
                        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode));
                }
            }
        } else if entry_type.is_symlink() {
            #[cfg(unix)]
            if let Ok(link_name) = entry.link_name() {
                if let Some(link_target) = link_name {
                    // Contain symlink target: resolve relative to the symlink's own
                    // location (parent of dest_path) and check it stays within dest_dir.
                    let symlink_parent = dest_path.parent().unwrap_or(dest_dir);
                    let resolved_target = symlink_parent.join(&*link_target);
                    let contained_target = path_is_within(dest_dir, &resolved_target)?;
                    // Re-derive the relative link target that points to the contained path
                    // from the symlink's parent, so the symlink itself is relative.
                    let relative_target = contained_target
                        .strip_prefix(symlink_parent)
                        .unwrap_or(&*link_target);
                    std::os::unix::fs::symlink(relative_target, &dest_path).map_err(|e| {
                        RepositoryExecutionError::ExecutionFailed {
                            name: "extract".to_owned(),
                            reason: format!("Failed to create symlink: {}", e),
                        }
                    })?;
                }
            }
        } else if entry_type.is_hard_link() {
            let link_name =
                entry
                    .link_name()
                    .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                        name: "extract".to_owned(),
                        reason: e.to_string(),
                    })?;
            if let Some(link_target) = link_name {
                let source_path = resolve_tar_link_target(&link_target, dest_dir, strip_prefix)?;
                std::fs::hard_link(&source_path, &dest_path)
                    .or_else(|_| std::fs::copy(&source_path, &dest_path).map(|_| ()))
                    .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                        name: "extract".to_owned(),
                        reason: format!(
                            "Failed to materialize hard link {:?} -> {:?}: {}",
                            dest_path, source_path, e
                        ),
                    })?;
            }
        }
    }

    Ok(())
}

/// Resolve a tar hard-link target, applying strip_prefix if present and
/// running the result through path containment.
fn resolve_tar_link_target(
    link_target: &Path,
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> slug_error::Result<PathBuf> {
    let target_str = link_target.to_string_lossy();
    let candidate = if let Some(prefix) = strip_prefix {
        if let Some(stripped) = target_str.strip_prefix(prefix) {
            PathBuf::from(stripped.trim_start_matches('/'))
        } else {
            let prefix_with_slash = format!("{}/", prefix.trim_end_matches('/'));
            if let Some(stripped) = target_str.strip_prefix(&prefix_with_slash) {
                PathBuf::from(stripped)
            } else {
                link_target.to_owned()
            }
        }
    } else {
        link_target.to_owned()
    };
    contain_path(dest_dir, &candidate)
}

/// Extract tar.gz archive.
fn extract_tar_gz(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> slug_error::Result<()> {
    extract_tar_from_reader(GzDecoder::new(data), dest_dir, strip_prefix)
}

/// Extract tar.xz archive.
fn extract_tar_xz(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> slug_error::Result<()> {
    extract_tar_from_reader(xz2::read::XzDecoder::new(data), dest_dir, strip_prefix)
}

/// Extract tar.zst archive.
fn extract_tar_zst(
    data: &[u8],
    dest_dir: &Path,
    strip_prefix: Option<&str>,
) -> slug_error::Result<()> {
    let decoder = zstd::stream::read::Decoder::new(data).map_err(|e| {
        RepositoryExecutionError::ExecutionFailed {
            name: "extract".to_owned(),
            reason: e.to_string(),
        }
    })?;
    extract_tar_from_reader(decoder, dest_dir, strip_prefix)
}

/// Extract zip archive.
fn extract_zip(data: &[u8], dest_dir: &Path, strip_prefix: Option<&str>) -> slug_error::Result<()> {
    let cursor = Cursor::new(data);
    let mut archive =
        ZipArchive::new(cursor).map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "extract".to_owned(),
            reason: e.to_string(),
        })?;

    for i in 0..archive.len() {
        let mut file =
            archive
                .by_index(i)
                .map_err(|e| RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: e.to_string(),
                })?;

        let file_path = match file.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };

        // Apply strip_prefix (enclosed_name() already strips .., but we also
        // run through contain_path as a belt-and-suspenders check)
        let dest_path = if let Some(prefix) = strip_prefix {
            let stripped = file_path.strip_prefix(prefix).unwrap_or(&file_path);
            contain_path(dest_dir, stripped)?
        } else {
            contain_path(dest_dir, &file_path)?
        };

        if dest_path == dest_dir {
            continue;
        }

        if file.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to create directory: {}", e),
                }
            })?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    RepositoryExecutionError::ExecutionFailed {
                        name: "extract".to_owned(),
                        reason: format!("Failed to create parent directory: {}", e),
                    }
                })?;
            }

            let mut outfile = std::fs::File::create(&dest_path).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to create file: {}", e),
                }
            })?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: "extract".to_owned(),
                    reason: format!("Failed to write file contents: {}", e),
                }
            })?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    let _ =
                        std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(mode));
                }
            }
        }
    }

    Ok(())
}

/// Execute git_repository rule.
fn execute_git_repository(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
) -> slug_error::Result<()> {
    let remote = attrs.require_string("remote")?;
    let commit = attrs.get_optional_string("commit");
    let tag = attrs.get_optional_string("tag");
    let branch = attrs.get_optional_string("branch");

    // Determine what to checkout
    let checkout_ref = commit.or(tag).or(branch).unwrap_or("HEAD");

    tracing::info!("Cloning git repository {} at {}", remote, checkout_ref);

    // Initialize git repo
    run_git(working_dir, |c| {
        c.arg("init");
    })?;

    // Add remote
    run_git(working_dir, |c| {
        c.arg("remote").arg("add").arg("origin").arg(remote);
    })?;

    // Fetch
    run_git(working_dir, |c| {
        c.arg("fetch").arg("origin").arg(checkout_ref);
    })?;

    // Checkout
    run_git(working_dir, |c| {
        c.arg("reset").arg("--hard").arg("FETCH_HEAD");
    })?;

    // Plan 39: keep `.git`. Downstream rules — most prominently
    // rules_rs's `crate_git_repository` — use `git --git-dir=<>/.git
    // worktree add` to fan one master clone out into per-crate spokes,
    // and that fails if we strip the directory here.

    // Create WORKSPACE if not present
    if !working_dir.join("WORKSPACE").exists() && !working_dir.join("WORKSPACE.bazel").exists() {
        std::fs::write(
            working_dir.join("WORKSPACE.bazel"),
            format!("workspace(name = \"{}\")\n", invocation.name),
        )
        .ok();
    }

    Ok(())
}

/// Run a git command.
fn run_git(cwd: &Path, configure: impl FnOnce(&mut Command)) -> slug_error::Result<()> {
    let mut cmd = Command::new("git");
    configure(&mut cmd);
    cmd.current_dir(cwd);

    let output = cmd
        .output()
        .map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "git".to_owned(),
            reason: format!("Failed to run git: {}", e),
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(RepositoryExecutionError::ExecutionFailed {
            name: "git".to_owned(),
            reason: format!("Git command failed: {}", stderr),
        }
        .into())
    }
}

/// Execute local_repository or new_local_repository rule.
fn execute_local_repository(
    invocation: &RepositoryInvocation,
    attrs: &InvocationAttrs,
    working_dir: &Path,
) -> slug_error::Result<()> {
    let path = attrs.require_string("path")?;

    // Resolve relative paths against the project root (parent of bazel-external/).
    // In Bazel, relative paths in new_local_repository are resolved relative to
    // the workspace root (where MODULE.bazel lives).
    let resolved_path = if Path::new(path).is_relative() {
        if let Some(bazel_external) = working_dir.parent() {
            if let Some(project_root) = bazel_external.parent() {
                project_root.join(path)
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    };

    let resolved_path = resolved_path
        .canonicalize()
        .unwrap_or_else(|_| resolved_path.clone());

    if invocation.rule_name == "new_local_repository" {
        // For new_local_repository: create working dir with symlinks to individual
        // entries from the target, plus a custom BUILD.bazel. Don't symlink the
        // directory itself (that would write BUILD.bazel into the source tree).
        std::fs::create_dir_all(working_dir).ok();

        // Symlink all entries from the target directory.
        // When path points to an ancestor of the working dir (e.g. "../.."),
        // symlinking everything would recurse into ourselves. We don't have
        // a generic solution, but excluding known slug output dirs covers the
        // common case (Bazel's llvm-raw pattern).
        if let Ok(entries) = std::fs::read_dir(&resolved_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                // Skip slug/bazel output dirs to avoid recursive self-reference
                if matches!(
                    name_str.as_ref(),
                    "bazel-external"
                        | "bazel-out"
                        | "bazel-bin"
                        | "bazel-testlogs"
                        | "buck-out"
                        | ".slug_repo_complete"
                ) {
                    continue;
                }
                let target = working_dir.join(&name);
                if !target.exists() {
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(entry.path(), &target).ok();
                    #[cfg(not(unix))]
                    {
                        if entry.path().is_dir() {
                            copy_dir_recursive(&entry.path(), &target).ok();
                        } else {
                            std::fs::copy(entry.path(), &target).ok();
                        }
                    }
                }
            }
        }

        // Write custom BUILD file
        if let Some(content) = attrs.get_optional_string("build_file_content") {
            // Remove any symlinked BUILD files first
            std::fs::remove_file(working_dir.join("BUILD.bazel")).ok();
            std::fs::remove_file(working_dir.join("BUILD")).ok();
            std::fs::write(working_dir.join("BUILD.bazel"), content).ok();
        }
    } else {
        // For local_repository: symlink the entire directory (it has its own BUILD)
        #[cfg(unix)]
        {
            std::fs::remove_dir(working_dir).ok();
            std::os::unix::fs::symlink(&resolved_path, working_dir).map_err(|e| {
                RepositoryExecutionError::ExecutionFailed {
                    name: invocation.name.clone(),
                    reason: format!(
                        "Failed to create symlink {} -> {}: {}",
                        working_dir.display(),
                        resolved_path.display(),
                        e
                    ),
                }
            })?;
        }
        #[cfg(not(unix))]
        {
            copy_dir_recursive(&resolved_path, working_dir)?;
        }
    }

    Ok(())
}

#[cfg(not(unix))]
fn copy_dir_recursive(src: &Path, dst: &Path) -> slug_error::Result<()> {
    std::fs::create_dir_all(dst).ok();

    for entry in std::fs::read_dir(src).map_err(|e| RepositoryExecutionError::ExecutionFailed {
        name: "copy".to_owned(),
        reason: e.to_string(),
    })? {
        let entry = entry.map_err(|e| RepositoryExecutionError::ExecutionFailed {
            name: "copy".to_owned(),
            reason: e.to_string(),
        })?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path).ok();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::Arc;

    use tempfile::TempDir;

    use super::*;
    use crate::dice_graph::BzlmodCellGraphAlias;
    use crate::dice_graph::BzlmodCellGraphCell;
    use crate::dice_graph::BzlmodCellGraphValue;
    use crate::dice_graph::WorkspaceId;

    fn create_hard_link_tar_gz(strip_prefix: Option<&str>) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;

        let mut builder = tar::Builder::new(Vec::new());
        let prefix = strip_prefix.unwrap_or("");
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{prefix}/")
        };

        let content = b"multicall";
        let original = format!("{prefix}bin/tool.exe");
        let link = format!("{prefix}bin/tool-alias.exe");
        let mut header = tar::Header::new_gnu();
        header.set_path(&original).unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();

        let mut link_header = tar::Header::new_gnu();
        link_header.set_entry_type(tar::EntryType::Link);
        link_header.set_path(&link).unwrap();
        link_header.set_link_name(&original).unwrap();
        link_header.set_size(0);
        link_header.set_mode(0o755);
        link_header.set_cksum();
        builder.append(&link_header, std::io::empty()).unwrap();

        let tar_data = builder.into_inner().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar_data).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn test_prepare_working_dir() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("test_repo");

        prepare_working_dir(&working_dir).unwrap();
        assert!(working_dir.exists());
        assert!(working_dir.is_dir());
    }

    #[test]
    fn test_is_repo_complete() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("test_repo");
        std::fs::create_dir_all(&working_dir).unwrap();

        assert!(!is_repo_complete(&working_dir));

        mark_repo_complete(&working_dir).unwrap();
        assert!(is_repo_complete(&working_dir));
    }

    #[test]
    fn fresh_repository_execution_bypasses_marker_shortcut() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("bazel-external/stale_repo");
        std::fs::create_dir_all(&working_dir).unwrap();
        mark_repo_complete(&working_dir).unwrap();

        let invocation =
            RepositoryInvocation::new("stale_repo".to_owned(), "unimplemented_rule".to_owned());
        let reused = execute_repository_rule(&invocation, temp.path()).unwrap();
        assert!(reused.success);

        let err = execute_repository_rule_fresh(&invocation, temp.path()).unwrap_err();
        assert!(
            err.to_string()
                .contains("Repository rule 'unimplemented_rule' has no implementation")
        );
        // With staging-dir materialization, a failed fresh execution cleans up
        // the staging dir but leaves the existing canonical directory intact.
        // This preserves the prior materialization so the repo isn't left empty.
        assert!(working_dir.exists());
    }

    #[test]
    fn repo_complete_marker_tracks_output_digest() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("test_repo");
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(working_dir.join("data.txt"), "fresh").unwrap();

        mark_repo_complete(&working_dir).unwrap();
        assert!(is_repo_complete(&working_dir));

        std::fs::write(working_dir.join("data.txt"), "corrupt").unwrap();
        assert!(!is_repo_complete(&working_dir));
    }

    #[test]
    fn repository_output_digest_ignores_completion_marker() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("test_repo");
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(working_dir.join("data.txt"), "fresh").unwrap();

        let digest = repository_output_digest(&working_dir).unwrap();
        std::fs::write(working_dir.join(".slug_repo_complete"), "complete:legacy").unwrap();
        std::fs::write(
            working_dir.join(REPO_RECORDED_INPUTS_FILE),
            "FILE:/tmp/does-not-exist ENOENT\n",
        )
        .unwrap();
        assert_eq!(repository_output_digest(&working_dir).unwrap(), digest);

        std::fs::write(working_dir.join("data.txt"), "changed").unwrap();
        assert_ne!(repository_output_digest(&working_dir).unwrap(), digest);
    }

    #[test]
    fn git_repository_marker_requires_git_layout() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("git_repo");
        std::fs::create_dir_all(&working_dir).unwrap();
        mark_repo_complete(&working_dir).unwrap();

        let git_inv = RepositoryInvocation::new("git_repo".to_owned(), "git_repository".to_owned());
        assert!(!repo_layout_is_valid_for_invocation(&git_inv, &working_dir));

        std::fs::create_dir(working_dir.join(".git")).unwrap();
        assert!(repo_layout_is_valid_for_invocation(&git_inv, &working_dir));

        let archive_inv =
            RepositoryInvocation::new("archive_repo".to_owned(), "http_archive".to_owned());
        assert!(repo_layout_is_valid_for_invocation(
            &archive_inv,
            &working_dir
        ));
    }

    #[test]
    fn new_local_repository_marker_requires_source_layout() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        std::fs::create_dir_all(source_dir.join("src")).unwrap();
        std::fs::write(source_dir.join("src/lib.h"), "header").unwrap();
        std::fs::write(source_dir.join("README.md"), "readme").unwrap();

        let working_dir = temp.path().join("bazel-external/local_repo");
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(working_dir.join("BUILD.bazel"), "# generated build").unwrap();
        mark_repo_complete(&working_dir).unwrap();

        let local_inv =
            RepositoryInvocation::new("local_repo".to_owned(), "new_local_repository".to_owned())
                .with_attr(
                    "path".to_owned(),
                    crate::repository_invocations::AttrValue::String(
                        source_dir.to_string_lossy().to_string(),
                    ),
                );

        assert!(!repo_layout_is_valid_for_invocation(
            &local_inv,
            &working_dir
        ));

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source_dir.join("src"), working_dir.join("src")).unwrap();
            std::os::unix::fs::symlink(source_dir.join("README.md"), working_dir.join("README.md"))
                .unwrap();
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir(working_dir.join("src")).unwrap();
            std::fs::write(working_dir.join("README.md"), "readme").unwrap();
        }
        assert!(repo_layout_is_valid_for_invocation(
            &local_inv,
            &working_dir
        ));

        #[cfg(unix)]
        {
            std::fs::remove_file(working_dir.join("README.md")).unwrap();
            std::fs::write(working_dir.join("README.md"), "corrupt").unwrap();
            assert!(!repo_layout_is_valid_for_invocation(
                &local_inv,
                &working_dir
            ));
        }
    }

    #[test]
    fn local_repository_marker_requires_repo_root_link() {
        let temp = TempDir::new().unwrap();
        let source_dir = temp.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("BUILD.bazel"),
            "filegroup(name = \"data\")\n",
        )
        .unwrap();
        std::fs::write(
            source_dir.join("MODULE.bazel"),
            "module(name = \"source\")\n",
        )
        .unwrap();
        std::fs::write(source_dir.join(".slug_repo_complete"), "complete").unwrap();

        let working_dir = temp.path().join("bazel-external/local_repo");
        std::fs::create_dir_all(working_dir.parent().unwrap()).unwrap();

        let local_inv =
            RepositoryInvocation::new("local_repo".to_owned(), "local_repository".to_owned())
                .with_attr(
                    "path".to_owned(),
                    crate::repository_invocations::AttrValue::String(
                        source_dir.to_string_lossy().to_string(),
                    ),
                );

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&source_dir, &working_dir).unwrap();
            assert!(repo_layout_is_valid_for_invocation(
                &local_inv,
                &working_dir
            ));

            std::fs::remove_file(&working_dir).unwrap();
            std::fs::create_dir(&working_dir).unwrap();
            std::fs::write(
                working_dir.join("BUILD.bazel"),
                "filegroup(name = \"data\")\n",
            )
            .unwrap();
            assert!(!repo_layout_is_valid_for_invocation(
                &local_inv,
                &working_dir
            ));
        }

        #[cfg(not(unix))]
        {
            std::fs::create_dir(&working_dir).unwrap();
            assert!(repo_layout_is_valid_for_invocation(
                &local_inv,
                &working_dir
            ));
        }
    }

    #[test]
    fn llvm_subproject_marker_requires_source_layout() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let raw_dir = project_root
            .join("bazel-external")
            .join("llvm++llvm_source+llvm-raw")
            .join("libcxx");
        std::fs::create_dir_all(raw_dir.join("src")).unwrap();
        std::fs::create_dir_all(raw_dir.join("include")).unwrap();

        let working_dir = project_root
            .join("bazel-external")
            .join("llvm++llvm_source+libcxx");
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(working_dir.join("BUILD.bazel"), "# generated build").unwrap();
        mark_repo_complete(&working_dir).unwrap();

        let invocation = RepositoryInvocation::new(
            "llvm++llvm_source+libcxx".to_owned(),
            "_llvm_subproject_repository".to_owned(),
        )
        .with_attr(
            "dir".to_owned(),
            crate::repository_invocations::AttrValue::String("libcxx".to_owned()),
        );

        assert!(!repo_layout_is_valid_for_invocation(
            &invocation,
            &working_dir
        ));

        #[cfg(unix)]
        {
            std::fs::create_dir(working_dir.join("src")).unwrap();
            std::fs::create_dir(working_dir.join("include")).unwrap();
            assert!(!repo_layout_is_valid_for_invocation(
                &invocation,
                &working_dir
            ));
            std::fs::remove_dir(working_dir.join("src")).unwrap();
            std::fs::remove_dir(working_dir.join("include")).unwrap();
            std::os::unix::fs::symlink(raw_dir.join("src"), working_dir.join("src")).unwrap();
            std::os::unix::fs::symlink(raw_dir.join("include"), working_dir.join("include"))
                .unwrap();
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir(working_dir.join("src")).unwrap();
            std::fs::create_dir(working_dir.join("include")).unwrap();
        }
        assert!(repo_layout_is_valid_for_invocation(
            &invocation,
            &working_dir
        ));
    }

    #[test]
    fn test_get_urls() {
        let mut inv = RepositoryInvocation::new("test".to_owned(), "http_archive".to_owned());

        // Single url
        inv.attrs.insert(
            "url".to_owned(),
            crate::repository_invocations::AttrValue::String(
                "https://example.com/a.tar.gz".to_owned(),
            ),
        );

        let attrs = InvocationAttrs::new(&inv);
        let urls = get_urls(&attrs).unwrap();
        assert_eq!(urls, vec!["https://example.com/a.tar.gz"]);

        // Multiple urls
        inv.attrs.insert(
            "urls".to_owned(),
            crate::repository_invocations::AttrValue::StringList(vec![
                "https://example.com/b.tar.gz".to_owned(),
                "https://example.com/c.tar.gz".to_owned(),
            ]),
        );

        let attrs = InvocationAttrs::new(&inv);
        let urls = get_urls(&attrs).unwrap();
        assert_eq!(
            urls,
            vec![
                "https://example.com/a.tar.gz",
                "https://example.com/b.tar.gz",
                "https://example.com/c.tar.gz"
            ]
        );
    }

    #[test]
    fn test_verify_sha256() {
        let data = b"Hello, World!";
        let hash = Sha256::digest(data);
        let expected = hex::encode(hash);

        assert!(verify_sha256(data, &expected).is_ok());
        assert!(verify_sha256(data, "wrong_hash").is_err());
    }

    #[test]
    fn test_extract_tar_gz_materializes_hard_links() {
        let temp_dir = TempDir::new().unwrap();
        let dest = temp_dir.path().join("extracted");
        std::fs::create_dir(&dest).unwrap();

        let data = create_hard_link_tar_gz(Some("toolchain"));
        extract_tar_gz(&data, &dest, Some("toolchain")).unwrap();

        assert_eq!(
            std::fs::read(dest.join("bin/tool.exe")).unwrap(),
            b"multicall"
        );
        assert_eq!(
            std::fs::read(dest.join("bin/tool-alias.exe")).unwrap(),
            b"multicall"
        );
    }

    #[test]
    fn test_materialize_llvm_multicall_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let repo = temp_dir.path();
        std::fs::create_dir(repo.join("bin")).unwrap();
        std::fs::write(
            repo.join("BUILD.bazel"),
            "load(\"@llvm//toolchain/llvm:llvm.bzl\", \"declare_llvm_targets\")\ndeclare_llvm_targets(suffix = \".exe\")\n",
        )
        .unwrap();
        std::fs::write(repo.join("bin/llvm.exe"), b"multicall").unwrap();

        materialize_llvm_multicall_aliases(repo);

        assert_eq!(
            std::fs::read(repo.join("bin/llvm-profdata.exe")).unwrap(),
            b"multicall"
        );
    }

    #[test]
    fn resolve_build_file_label_uses_canonical_label_parser() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let repo_root = project_root.join("bazel-external").join("rules_cc");
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(repo_root.join("cc")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(repo_root.join("cc").join("BUILD.rules"), "").unwrap();
        let mut cell_graph = BzlmodCellGraphValue::empty_for_workspace(
            WorkspaceId::for_project_root(project_root.to_path_buf()),
        );
        cell_graph.cells = Arc::new(vec![BzlmodCellGraphCell {
            name: "rules_cc".to_owned(),
            path: "bazel-external/rules_cc".to_owned(),
            module_setup: None,
            bundled: false,
        }]);
        let label_resolution =
            RepositoryLabelResolution::from_cell_graph(project_root, &cell_graph);

        let resolved = resolve_build_file_label(
            "@@rules_cc//cc:BUILD.rules",
            &working_dir,
            &label_resolution,
        )
        .unwrap();

        assert_eq!(
            PathBuf::from(resolved),
            repo_root.join("cc").join("BUILD.rules")
        );
    }

    #[test]
    fn resolve_build_file_label_keeps_plain_paths_plain() {
        let temp = TempDir::new().unwrap();
        let working_dir = temp.path().join("bazel-external").join("current_repo");
        let label_resolution = RepositoryLabelResolution::default();

        assert_eq!(
            resolve_build_file_label("third_party/BUILD.foo", &working_dir, &label_resolution)
                .unwrap(),
            "third_party/BUILD.foo"
        );
    }

    #[test]
    fn resolve_build_file_label_supports_main_repo_labels() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(project_root.join("tools")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(project_root.join("tools").join("BUILD.repo"), "").unwrap();
        let label_resolution = RepositoryLabelResolution::default();

        let resolved =
            resolve_build_file_label("//tools:BUILD.repo", &working_dir, &label_resolution)
                .unwrap();

        assert_eq!(
            PathBuf::from(resolved),
            project_root.join("tools").join("BUILD.repo")
        );
    }

    #[test]
    fn resolve_build_file_label_supports_graph_root_module_labels() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(project_root.join("tools")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(project_root.join("tools").join("BUILD.repo"), "").unwrap();
        let mut cell_graph = BzlmodCellGraphValue::empty_for_workspace(
            WorkspaceId::for_project_root(project_root.to_path_buf()),
        );
        cell_graph.root_module_name = "root_module".to_owned();
        let label_resolution =
            RepositoryLabelResolution::from_cell_graph(project_root, &cell_graph);

        let resolved = resolve_build_file_label(
            "@@root_module//tools:BUILD.repo",
            &working_dir,
            &label_resolution,
        )
        .unwrap();

        assert_eq!(
            PathBuf::from(resolved),
            project_root.join("tools").join("BUILD.repo")
        );
    }

    #[test]
    fn resolve_build_file_label_quarantines_bazel_external_scan_fallback() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let legacy_repo = project_root.join("bazel-external").join("rules_cc+0.1.0");
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(legacy_repo.join("cc")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(legacy_repo.join("cc").join("BUILD.rules"), "").unwrap();
        let label_resolution = RepositoryLabelResolution::default();

        let err =
            resolve_build_file_label("@rules_cc//cc:BUILD.rules", &working_dir, &label_resolution)
                .unwrap_err();

        assert!(format!("{err:#}").contains("resolver-owned bzlmod cell graph"));
    }

    #[test]
    fn resolve_build_file_label_prefers_resolver_owned_cell_paths() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let stale_repo = project_root.join("bazel-external").join("rules_cc");
        let graph_repo = project_root.join("bazel-external").join("rules_cc+0.2.17");
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(stale_repo.join("cc")).unwrap();
        std::fs::create_dir_all(graph_repo.join("cc")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(stale_repo.join("cc").join("BUILD.rules"), "stale").unwrap();
        std::fs::write(graph_repo.join("cc").join("BUILD.rules"), "current").unwrap();

        let mut cell_graph = BzlmodCellGraphValue::empty_for_workspace(
            WorkspaceId::for_project_root(project_root.to_path_buf()),
        );
        cell_graph.cells = Arc::new(vec![BzlmodCellGraphCell {
            name: "rules_cc+0.2.17".to_owned(),
            path: "bazel-external/rules_cc+0.2.17".to_owned(),
            module_setup: None,
            bundled: false,
        }]);
        cell_graph.root_aliases = Arc::new(vec![BzlmodCellGraphAlias {
            apparent_name: "rules_cc".to_owned(),
            target_name: "rules_cc+0.2.17".to_owned(),
        }]);
        let label_resolution =
            RepositoryLabelResolution::from_cell_graph(project_root, &cell_graph);

        let resolved =
            resolve_build_file_label("@rules_cc//cc:BUILD.rules", &working_dir, &label_resolution)
                .unwrap();

        assert_eq!(
            PathBuf::from(resolved),
            graph_repo.join("cc").join("BUILD.rules")
        );
    }

    #[test]
    fn resolve_build_file_label_supports_graph_owned_canonical_module_repo() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let graph_repo = project_root.join("bazel-external").join("rules_cc+");
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(graph_repo.join("cc")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(graph_repo.join("cc").join("BUILD.rules"), "current").unwrap();

        let mut cell_graph = BzlmodCellGraphValue::empty_for_workspace(
            WorkspaceId::for_project_root(project_root.to_path_buf()),
        );
        cell_graph.cells = Arc::new(vec![BzlmodCellGraphCell {
            name: "rules_cc".to_owned(),
            path: "bazel-external/rules_cc+".to_owned(),
            module_setup: None,
            bundled: false,
        }]);
        let label_resolution =
            RepositoryLabelResolution::from_cell_graph(project_root, &cell_graph);

        let resolved = resolve_build_file_label(
            "@@rules_cc+//cc:BUILD.rules",
            &working_dir,
            &label_resolution,
        )
        .unwrap();

        assert_eq!(
            PathBuf::from(resolved),
            graph_repo.join("cc").join("BUILD.rules")
        );
    }

    #[test]
    fn resolve_build_file_label_resolver_owned_miss_rejects_legacy_collisions() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let source_tree_repo = project_root.join("rules_cc");
        let stale_exact_repo = project_root.join("bazel-external").join("rules_cc");
        let legacy_repo = project_root.join("bazel-external").join("rules_cc+0.1.0");
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(source_tree_repo.join("cc")).unwrap();
        std::fs::create_dir_all(stale_exact_repo.join("cc")).unwrap();
        std::fs::create_dir_all(legacy_repo.join("cc")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(source_tree_repo.join("cc").join("BUILD.rules"), "").unwrap();
        std::fs::write(stale_exact_repo.join("cc").join("BUILD.rules"), "").unwrap();
        std::fs::write(legacy_repo.join("cc").join("BUILD.rules"), "").unwrap();

        let cell_graph = BzlmodCellGraphValue::empty_for_workspace(WorkspaceId::for_project_root(
            project_root.to_path_buf(),
        ));
        let label_resolution =
            RepositoryLabelResolution::from_cell_graph(project_root, &cell_graph);

        let err =
            resolve_build_file_label("@rules_cc//cc:BUILD.rules", &working_dir, &label_resolution)
                .unwrap_err();

        assert!(format!("{err:#}").contains("resolver-owned bzlmod cell graph"));
    }

    #[test]
    fn resolve_build_file_label_requires_explicit_graph_alias_for_module_name() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let graph_repo = project_root.join("bazel-external").join("rules_cc+0.2.17");
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(graph_repo.join("cc")).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(graph_repo.join("cc").join("BUILD.rules"), "").unwrap();

        let mut cell_graph = BzlmodCellGraphValue::empty_for_workspace(
            WorkspaceId::for_project_root(project_root.to_path_buf()),
        );
        cell_graph.cells = Arc::new(vec![BzlmodCellGraphCell {
            name: "rules_cc+0.2.17".to_owned(),
            path: "bazel-external/rules_cc+0.2.17".to_owned(),
            module_setup: None,
            bundled: false,
        }]);
        let label_resolution =
            RepositoryLabelResolution::from_cell_graph(project_root, &cell_graph);

        let err =
            resolve_build_file_label("@rules_cc//cc:BUILD.rules", &working_dir, &label_resolution)
                .unwrap_err();

        assert!(format!("{err:#}").contains("resolver-owned bzlmod cell graph"));
    }

    #[test]
    fn resolve_build_file_label_does_not_treat_internal_names_as_global_aliases() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let canonical = "_main+ext+generated";
        let internal = "generated";
        let graph_repo = project_root.join("bazel-external").join(canonical);
        let working_dir = project_root.join("bazel-external").join("current_repo");
        std::fs::create_dir_all(&graph_repo).unwrap();
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::write(graph_repo.join("BUILD.repo"), "").unwrap();

        let mut cell_graph = BzlmodCellGraphValue::empty_for_workspace(
            WorkspaceId::for_project_root(project_root.to_path_buf()),
        );
        cell_graph.extension_cells =
            Arc::new(vec![crate::dice_graph::BzlmodCellGraphExtensionCell {
                canonical_name: canonical.to_owned(),
                internal_name: internal.to_owned(),
                path: format!("bazel-external/{canonical}"),
                extension_id: "@_main//:ext.bzl%ext".to_owned(),
                spec_hash: String::new(),
                repo_spec_json: String::new(),
                repo_env_json: String::new(),
                extension_usages_digest: String::new(),
                extension_replay_inputs_identity_digest: String::new(),
                extension_repo_mappings_digest: String::new(),
                extension_repo_mapping_overrides_digest: String::new(),
                extension_bzl_transitive_digest: String::new(),
                extension_recorded_inputs_json: String::new(),
                materialized: false,
                lazy: false,
            }]);
        let label_resolution =
            RepositoryLabelResolution::from_cell_graph(project_root, &cell_graph);

        let internal_resolved = resolve_build_file_label(
            &format!("@{internal}//:BUILD.repo"),
            &working_dir,
            &label_resolution,
        );
        assert!(internal_resolved.is_err());

        let canonical_resolved = resolve_build_file_label(
            &format!("@@{canonical}//:BUILD.repo"),
            &working_dir,
            &label_resolution,
        )
        .unwrap();
        assert_eq!(
            PathBuf::from(canonical_resolved),
            graph_repo.join("BUILD.repo")
        );
    }

    #[test]
    fn http_archive_build_file_uses_resolver_owned_label_path() {
        let temp = TempDir::new().unwrap();
        let project_root = temp.path();
        let archive_path = project_root.join("archive.tar.gz");
        std::fs::write(&archive_path, create_hard_link_tar_gz(None)).unwrap();

        let stale_repo = project_root.join("bazel-external").join("rules_cc");
        let graph_repo = project_root.join("bazel-external").join("rules_cc+0.2.17");
        std::fs::create_dir_all(stale_repo.join("cc")).unwrap();
        std::fs::create_dir_all(graph_repo.join("cc")).unwrap();
        std::fs::write(stale_repo.join("cc").join("BUILD.rules"), "# stale\n").unwrap();
        std::fs::write(
            graph_repo.join("cc").join("BUILD.rules"),
            "# resolver owned\n",
        )
        .unwrap();

        let mut cell_graph = BzlmodCellGraphValue::empty_for_workspace(
            WorkspaceId::for_project_root(project_root.to_path_buf()),
        );
        cell_graph.cells = Arc::new(vec![BzlmodCellGraphCell {
            name: "rules_cc+0.2.17".to_owned(),
            path: "bazel-external/rules_cc+0.2.17".to_owned(),
            module_setup: None,
            bundled: false,
        }]);
        cell_graph.root_aliases = Arc::new(vec![BzlmodCellGraphAlias {
            apparent_name: "rules_cc".to_owned(),
            target_name: "rules_cc+0.2.17".to_owned(),
        }]);
        let label_resolution =
            RepositoryLabelResolution::from_cell_graph(project_root, &cell_graph);
        let invocation =
            RepositoryInvocation::new("archive_repo".to_owned(), "http_archive".to_owned())
                .with_attr(
                    "url".to_owned(),
                    crate::repository_invocations::AttrValue::String(format!(
                        "file://{}",
                        archive_path.display()
                    )),
                )
                .with_attr(
                    "build_file".to_owned(),
                    crate::repository_invocations::AttrValue::Label(
                        "@rules_cc//cc:BUILD.rules".to_owned(),
                    ),
                );

        let result = execute_repository_rule_fresh_with_label_resolution(
            &invocation,
            project_root,
            &label_resolution,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(result.repo_path.join("BUILD.bazel")).unwrap(),
            "# resolver owned\n"
        );
    }

    // --- Phase 7: Path-traversal + symlink containment tests ---

    /// Helper: build an in-memory tar.gz archive from a closure that populates entries.
    fn build_tar_gz<F>(f: F) -> Vec<u8>
    where
        F: FnOnce(&mut tar::Builder<std::vec::Vec<u8>>),
    {
        use flate2::Compression;
        use flate2::write::GzEncoder;

        let mut builder = tar::Builder::new(Vec::new());
        f(&mut builder);
        let data = builder.into_inner().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&data).unwrap();
        encoder.finish().unwrap()
    }

    /// Helper: append a tar entry with an arbitrary raw path, bypassing the tar
    /// crate's path validation. This simulates a malicious archive that a real
    /// attacker could craft.
    fn append_raw_path_entry(
        builder: &mut tar::Builder<Vec<u8>>,
        entry_type: tar::EntryType,
        path: &[u8],
        link_target: Option<&[u8]>,
        data: &[u8],
    ) {
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_entry_type(entry_type);
        // Write path and link target directly into raw GNU header fields
        // to bypass the tar crate's path validation.
        if let Some(gnu) = header.as_gnu_mut() {
            let path_len = path.len().min(99);
            gnu.name[..path_len].copy_from_slice(&path[..path_len]);
            gnu.name[path_len] = 0;
            if let Some(target) = link_target {
                let target_len = target.len().min(99);
                gnu.linkname[..target_len].copy_from_slice(&target[..target_len]);
                gnu.linkname[target_len] = 0;
            }
        }
        header.set_cksum();
        builder.append(&mut header, data).unwrap();
    }

    #[test]
    fn test_path_traversal_dotdot() {
        let dest = tempfile::tempdir().unwrap();
        let data = build_tar_gz(|builder| {
            append_raw_path_entry(
                builder,
                tar::EntryType::file(),
                b"../escape.txt",
                None,
                b"evil\n",
            );
        });
        let result = extract_tar_gz(&data, dest.path(), None);
        assert!(result.is_err(), "Expected error for path traversal via ../");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("escapes destination directory"),
            "Error message should mention path traversal: {err_msg}"
        );
    }

    #[test]
    fn test_path_traversal_absolute() {
        let dest = tempfile::tempdir().unwrap();
        let data = build_tar_gz(|builder| {
            append_raw_path_entry(
                builder,
                tar::EntryType::file(),
                b"/tmp/escape.txt",
                None,
                b"evil\n",
            );
        });
        let result = extract_tar_gz(&data, dest.path(), None);
        assert!(result.is_err(), "Expected error for absolute path entry");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("escapes destination directory"),
            "Error message should mention path traversal: {err_msg}"
        );
    }

    #[test]
    fn test_symlink_escape() {
        let dest = tempfile::tempdir().unwrap();
        let data = build_tar_gz(|builder| {
            // Create a directory so the symlink has a parent inside dest_dir
            let mut dir_header = tar::Header::new_gnu();
            dir_header.set_size(0);
            dir_header.set_entry_type(tar::EntryType::dir());
            dir_header.set_cksum();
            builder
                .append_data(&mut dir_header, "inner", std::io::empty())
                .unwrap();

            // Symlink that escapes via ../../
            append_raw_path_entry(
                builder,
                tar::EntryType::symlink(),
                b"inner/link",
                Some(b"../../escape"),
                &[],
            );
        });
        let result = extract_tar_gz(&data, dest.path(), None);
        assert!(result.is_err(), "Expected error for escaping symlink");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("escapes destination directory"),
            "Error message should mention path traversal: {err_msg}"
        );
    }

    #[test]
    fn test_hard_link_escape() {
        let dest = tempfile::tempdir().unwrap();
        let data = build_tar_gz(|builder| {
            // First a normal file entry
            let mut header = tar::Header::new_gnu();
            header.set_size(4);
            header.set_entry_type(tar::EntryType::file());
            header.set_cksum();
            builder
                .append_data(&mut header, "original.txt", &b"data"[..])
                .unwrap();

            // Hard-link whose target escapes via ../..
            append_raw_path_entry(
                builder,
                tar::EntryType::hard_link(),
                b"hl.txt",
                Some(b"../../etc/passwd"),
                &[],
            );
        });
        let result = extract_tar_gz(&data, dest.path(), None);
        assert!(result.is_err(), "Expected error for escaping hard link");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("escapes destination directory"),
            "Error message should mention path traversal: {err_msg}"
        );
    }

    #[test]
    fn test_in_repo_symlink_ok() {
        let dest = tempfile::tempdir().unwrap();
        let data = build_tar_gz(|builder| {
            // Create sibling directory and inner directory
            let mut dir_header = tar::Header::new_gnu();
            dir_header.set_size(0);
            dir_header.set_entry_type(tar::EntryType::dir());
            dir_header.set_cksum();
            builder
                .append_data(&mut dir_header, "sibling", std::io::empty())
                .unwrap();

            let mut dir_header2 = tar::Header::new_gnu();
            dir_header2.set_size(0);
            dir_header2.set_entry_type(tar::EntryType::dir());
            dir_header2.set_cksum();
            builder
                .append_data(&mut dir_header2, "inner", std::io::empty())
                .unwrap();

            // Symlink: inner/link -> ../sibling  (both inside dest_dir)
            let mut link_header = tar::Header::new_gnu();
            link_header.set_size(0);
            link_header.set_entry_type(tar::EntryType::symlink());
            link_header.set_cksum();
            builder
                .append_link(&mut link_header, "inner/link", "../sibling")
                .unwrap();
        });
        let result = extract_tar_gz(&data, dest.path(), None);
        assert!(result.is_ok(), "In-repo relative symlinks should be allowed");
        // Verify the symlink exists and points correctly
        let link_path = dest.path().join("inner/link");
        assert!(link_path.exists(), "Symlink should exist");
        assert!(
            std::fs::symlink_metadata(&link_path)
                .unwrap()
                .file_type()
                .is_symlink(),
            "Should be a symlink"
        );
    }

    #[test]
    fn test_normal_extraction_ok() {
        let dest = tempfile::tempdir().unwrap();
        let data = build_tar_gz(|builder| {
            // Directory
            let mut dir_header = tar::Header::new_gnu();
            dir_header.set_size(0);
            dir_header.set_entry_type(tar::EntryType::dir());
            dir_header.set_cksum();
            builder
                .append_data(&mut dir_header, "subdir", std::io::empty())
                .unwrap();

            // Regular file
            let mut file_header = tar::Header::new_gnu();
            file_header.set_size(11);
            file_header.set_entry_type(tar::EntryType::file());
            file_header.set_cksum();
            builder
                .append_data(&mut file_header, "subdir/hello.txt", &b"hello world"[..])
                .unwrap();
        });
        let result = extract_tar_gz(&data, dest.path(), None);
        assert!(result.is_ok(), "Normal extraction should succeed");
        let content =
            std::fs::read_to_string(dest.path().join("subdir/hello.txt")).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_contain_path_rejects_dotdot() {
        let dest = Path::new("/tmp/dest");
        let result = contain_path(dest, Path::new("../etc/passwd"));
        assert!(result.is_err());
    }

    #[test]
    fn test_contain_path_rejects_absolute() {
        let dest = Path::new("/tmp/dest");
        let result = contain_path(dest, Path::new("/etc/passwd"));
        assert!(result.is_err());
    }

    #[test]
    fn test_contain_path_allows_normal() {
        let dest = Path::new("/tmp/dest");
        let result = contain_path(dest, Path::new("foo/bar.txt"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/tmp/dest/foo/bar.txt"));
    }

    #[test]
    fn test_contain_path_normalizes_dotdot_in_middle() {
        let dest = Path::new("/tmp/dest");
        // foo/../bar should normalize to /tmp/dest/bar (still inside)
        let result = contain_path(dest, Path::new("foo/../bar"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/tmp/dest/bar"));
    }
}
