/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Per-action execroot narrowing — Plan 44 Phase 2.6.
//!
//! Builds a small directory under `<project_root>/execroot/<digest>/`
//! containing only the top-level path components an action's declared
//! inputs and tools require. The action runs with that directory as
//! its `cwd`, so `read_dir(cwd)` returns exactly the prefixes the
//! action needs — matching Bazel's exec_root invariant without
//! sandbox staging.
//!
//! Replaces the global allowlist-filtered execroot from Phase 2.5
//! (`slug_core::cells::ensure_execroot_layout`).

use std::collections::BTreeSet;
use std::hash::Hash;
use std::hash::Hasher;
use std::path::PathBuf;
use std::sync::Mutex;

use slug_core::fs::artifact_path_resolver::ArtifactFs;
use slug_execute::execute::request::CommandExecutionInput;
use slug_execute::execute::request::CommandExecutionRequest;
use slug_fs::paths::abs_norm_path::AbsNormPath;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;

/// Names that should always be available in the execroot regardless
/// of whether the action explicitly declared an input under them.
///
/// `buck-out` is needed because tool paths in command lines reference
/// `buck-out/v2/gen/...` directly (rules_rust runner, process wrapper,
/// rustc, etc.); without it the cwd-relative path can't resolve.
/// `external` is needed because the bzlmod apparent-name alias dir at
/// `<workspace>/external/<apparent>` is how slug routes
/// `external/<repo>/...` paths to the actual `bazel-external/...`
/// canonical repos.
const ALWAYS_INCLUDE_PREFIXES: &[&str] = &["buck-out", "external"];

/// Compute the sorted set of top-level workspace path components
/// that an action's inputs and tools refer to.
///
/// Each component is the first segment of a project-relative path
/// (e.g. `buck-out/v2/gen/foo/bar` → `buck-out`,
/// `external/crates__zerocopy-0.8.42/src/lib.rs` → `external`,
/// `lib/units/build.rs` → `lib`).
pub(crate) fn collect_input_prefixes(
    request: &CommandExecutionRequest,
    artifact_fs: &ArtifactFs,
) -> BTreeSet<String> {
    let mut prefixes: BTreeSet<String> = ALWAYS_INCLUDE_PREFIXES
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    let inputs_iter = request.inputs().iter().chain(
        request
            .worker()
            .as_ref()
            .map(|w| w.inputs())
            .unwrap_or_default(),
    );

    for input in inputs_iter {
        match input {
            CommandExecutionInput::Artifact(group) => {
                for (artifact, _value) in group.iter() {
                    if let Ok(path) = artifact.resolve_configuration_hash_path(artifact_fs) {
                        if let Some(prefix) = top_level_component(path.as_str()) {
                            prefixes.insert(prefix.to_owned());
                        }
                    }
                }
            }
            CommandExecutionInput::IncrementalRemoteOutput(path, _) => {
                if let Some(prefix) = top_level_component(path.as_str()) {
                    prefixes.insert(prefix.to_owned());
                }
            }
            // Metadata blobs and scratch paths don't surface workspace
            // prefixes — they live under buck-out (already included).
            CommandExecutionInput::ActionMetadata(_) | CommandExecutionInput::ScratchPath(_) => {}
        }
    }

    prefixes
}

/// Extract the first path component of a project-relative path.
///
/// Returns `None` for empty paths or paths that escape the workspace
/// (defensive — `ProjectRelativePath` should never have those, but
/// the helper is paranoid).
fn top_level_component(path: &str) -> Option<&str> {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let first = trimmed.split('/').next()?;
    if first.is_empty() || first == "." || first == ".." {
        return None;
    }
    Some(first)
}

/// Stable digest for a sorted prefix set. Used as the per-action
/// execroot directory name. Not security-sensitive — just needs to
/// dedupe identical input shapes — so the standard hasher is fine.
fn digest_prefixes(prefixes: &BTreeSet<String>) -> String {
    // Use SipHasher with fixed keys for stability across processes
    // (the std DefaultHasher uses a randomized key).
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for prefix in prefixes {
        prefix.hash(&mut hasher);
        0u8.hash(&mut hasher); // separator
    }
    format!("{:016x}", hasher.finish())
}

/// Process-global cache of execroot directories that have already been
/// materialised this build. Keyed by digest so concurrent actions with
/// the same input shape share the same directory without re-doing
/// `mkdir`/`symlinkat` work.
static MATERIALIZED_EXECROOTS: Mutex<Option<MaterializedSet>> = Mutex::new(None);

struct MaterializedSet {
    project_root: PathBuf,
    digests: std::collections::HashSet<String>,
}

/// Build (or return the cached path of) a per-action execroot
/// containing top-level symlinks for every prefix in `prefixes`.
///
/// Path: `<project_root>/execroot/<digest>/`.
///
/// Mirrors Bazel's
/// `<output_base>/execroot/<workspace_name>/` layout so that
/// rules_rust's `process_wrapper` (which derives `${exec_root}` as
/// `<output_base>/execroot/<basename of cwd>`) resolves to the
/// directory that actually exists on disk. The directory is created
/// lazily; symlinks point to the workspace's `<project_root>/<prefix>/`
/// directory. If a prefix doesn't exist as a workspace directory it's
/// silently skipped — actions reference real paths only.
pub(crate) fn ensure_execroot(
    project_root: &AbsNormPath,
    prefixes: &BTreeSet<String>,
) -> Option<AbsNormPathBuf> {
    if prefixes.is_empty() {
        return None;
    }
    if prefixes.contains("external") {
        slug_core::cells::repair_external_symlink_targets(project_root.as_path());
    }

    let digest = digest_prefixes(prefixes);
    let execroot_abs: PathBuf = project_root.as_path().join("execroot").join(&digest);

    let mut guard = MATERIALIZED_EXECROOTS.lock().ok()?;
    let entry = guard.get_or_insert_with(|| MaterializedSet {
        project_root: project_root.as_path().to_path_buf(),
        digests: Default::default(),
    });
    // Reset cache if the project root changed (e.g. test isolation).
    if entry.project_root != project_root.as_path() {
        entry.project_root = project_root.as_path().to_path_buf();
        entry.digests.clear();
    }

    if entry.digests.contains(&digest) {
        return AbsNormPathBuf::new(execroot_abs).ok();
    }

    if let Err(e) = std::fs::create_dir_all(&execroot_abs) {
        tracing::debug!(?e, "failed to create per-action execroot dir; falling back");
        return None;
    }

    for prefix in prefixes {
        let target = project_root.as_path().join(prefix);
        if !target.exists() {
            continue;
        }
        let link = execroot_abs.join(prefix);
        if prefix == "external" {
            if !materialize_external_prefix(&target, &link) {
                return None;
            }
            continue;
        }
        match link.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                // Stale or wrong target — refresh.
                if !remove_symlink_path(&link) {
                    return None;
                }
            }
            Ok(_) => continue,
            Err(_) => {}
        }
        #[cfg(unix)]
        let r = std::os::unix::fs::symlink(&target, &link);
        #[cfg(windows)]
        let r = std::os::windows::fs::symlink_dir(&target, &link);
        if let Err(e) = r {
            // EEXIST race with another action populating the same
            // dir is fine — the symlink is content-equivalent
            // because `prefixes` derives from the same digest.
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                tracing::debug!(
                    ?e,
                    prefix = %prefix,
                    "failed to populate execroot symlink; falling back"
                );
                return None;
            }
        }
    }

    entry.digests.insert(digest);
    AbsNormPathBuf::new(execroot_abs).ok()
}

fn materialize_external_prefix(
    project_external: &std::path::Path,
    execroot_external: &std::path::Path,
) -> bool {
    match execroot_external.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            if !remove_symlink_path(execroot_external) {
                tracing::debug!(
                    path = %execroot_external.display(),
                    "failed to replace external prefix symlink with directory"
                );
                return false;
            }
        }
        Ok(meta) if !meta.is_dir() => return true,
        Ok(_) => {}
        Err(_) => {}
    }
    if let Err(e) = std::fs::create_dir_all(execroot_external) {
        tracing::debug!(
            ?e,
            path = %execroot_external.display(),
            "failed to create execroot external directory"
        );
        return false;
    }

    let entries = match std::fs::read_dir(project_external) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!(
                ?e,
                path = %project_external.display(),
                "failed to read project external directory"
            );
            return false;
        }
    };

    for entry in entries.flatten() {
        let target = canonical_external_entry_target(project_external, &entry.path())
            .unwrap_or_else(|| entry.path());
        if !link_external_entry(execroot_external, entry.file_name(), &target) {
            return false;
        }
    }

    if let Some(project_root) = project_external.parent() {
        let bazel_external = project_root.join("bazel-external");
        if let Ok(entries) = std::fs::read_dir(&bazel_external) {
            for entry in entries.flatten() {
                let target = std::fs::canonicalize(entry.path()).unwrap_or_else(|_| entry.path());
                if !link_external_entry(execroot_external, entry.file_name(), &target) {
                    return false;
                }
            }
        }
    }

    true
}

fn link_external_entry(
    execroot_external: &std::path::Path,
    name: std::ffi::OsString,
    target: &std::path::Path,
) -> bool {
    let link = execroot_external.join(name);
    match link.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            let current = std::fs::read_link(&link).ok();
            if current.as_deref() == Some(target) {
                return true;
            }
            if !remove_symlink_path(&link) {
                tracing::debug!(
                    link = %link.display(),
                    target = %target.display(),
                    "failed to remove stale execroot external repo symlink"
                );
                return false;
            }
        }
        Ok(_) => return true,
        Err(_) => {}
    }

    #[cfg(unix)]
    let r = std::os::unix::fs::symlink(target, &link);
    #[cfg(windows)]
    let r = std::os::windows::fs::symlink_dir(target, &link);
    if let Err(e) = r {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            tracing::debug!(
                ?e,
                link = %link.display(),
                target = %target.display(),
                "failed to populate execroot external repo symlink"
            );
            return false;
        }
    }
    true
}

fn remove_symlink_path(path: &std::path::Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(file_err) => match std::fs::remove_dir(path) {
            Ok(()) => true,
            Err(dir_err) => {
                tracing::debug!(
                    ?file_err,
                    ?dir_err,
                    path = %path.display(),
                    "failed to remove symlink"
                );
                false
            }
        },
    }
}

fn canonical_external_entry_target(
    project_external: &std::path::Path,
    entry_path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let metadata = std::fs::symlink_metadata(entry_path).ok()?;
    if !metadata.file_type().is_symlink() {
        return std::fs::canonicalize(entry_path).ok();
    }
    let target = std::fs::read_link(entry_path).ok()?;
    let target = if target.is_absolute() {
        target
    } else {
        project_external.join(target)
    };
    std::fs::canonicalize(target).ok()
}

/// Reset the materialised-execroot cache. Tests use this between
/// independent project roots to avoid cross-talk.
#[cfg(test)]
pub(crate) fn reset_cache_for_test() {
    if let Ok(mut guard) = MATERIALIZED_EXECROOTS.lock() {
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_component_strips_slashes() {
        assert_eq!(top_level_component(""), None);
        assert_eq!(top_level_component("/"), None);
        assert_eq!(top_level_component("/foo"), Some("foo"));
        assert_eq!(top_level_component("foo"), Some("foo"));
        assert_eq!(top_level_component("foo/bar"), Some("foo"));
        assert_eq!(top_level_component("foo/bar/baz"), Some("foo"));
        assert_eq!(top_level_component("./foo"), None);
    }

    #[test]
    fn digest_is_stable_and_set_independent_order() {
        let mut a = BTreeSet::new();
        a.insert("buck-out".to_owned());
        a.insert("external".to_owned());
        a.insert("lib".to_owned());

        let mut b = BTreeSet::new();
        b.insert("lib".to_owned());
        b.insert("external".to_owned());
        b.insert("buck-out".to_owned());

        assert_eq!(digest_prefixes(&a), digest_prefixes(&b));
    }

    #[test]
    fn digest_changes_with_prefix_set() {
        let mut a = BTreeSet::new();
        a.insert("buck-out".to_owned());

        let mut b = BTreeSet::new();
        b.insert("buck-out".to_owned());
        b.insert("external".to_owned());

        assert_ne!(digest_prefixes(&a), digest_prefixes(&b));
    }

    #[test]
    fn execroot_uses_workspace_layout() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().to_path_buf();
        std::fs::create_dir_all(project.join("buck-out")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.clone()).unwrap();

        let mut prefixes = BTreeSet::new();
        prefixes.insert("buck-out".to_owned());

        let execroot = ensure_execroot(&project_norm, &prefixes).unwrap();

        assert!(execroot.as_path().starts_with(project.join("execroot")));
    }

    #[test]
    fn ensure_execroot_creates_dir_with_symlinks() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir(project.join("buck-out")).unwrap();
        std::fs::create_dir(project.join("external")).unwrap();
        std::fs::create_dir(project.join("lib")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut prefixes = BTreeSet::new();
        prefixes.insert("buck-out".to_owned());
        prefixes.insert("external".to_owned());
        prefixes.insert("lib".to_owned());

        let exec = ensure_execroot(&project_norm, &prefixes).unwrap();

        assert!(exec.as_path().is_dir());
        assert!(exec.as_path().join("buck-out").is_dir());
        assert!(exec.as_path().join("external").is_dir());
        assert!(exec.as_path().join("lib").is_dir());

        // Workspace dirs not in the prefix set are absent.
        assert!(!exec.as_path().join("ci").exists());
    }

    #[test]
    fn ensure_execroot_skips_missing_workspace_dirs() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir(project.join("buck-out")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut prefixes = BTreeSet::new();
        prefixes.insert("buck-out".to_owned());
        prefixes.insert("does-not-exist".to_owned());

        let exec = ensure_execroot(&project_norm, &prefixes).unwrap();

        assert!(exec.as_path().join("buck-out").is_dir());
        assert!(!exec.as_path().join("does-not-exist").exists());
    }

    #[test]
    fn ensure_execroot_repairs_external_symlink_chains() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        let external = project.join("external");
        let bazel_external = project.join("bazel-external");
        let cache_repo = project.join("cache").join("rules_rust");
        std::fs::create_dir_all(&external).unwrap();
        std::fs::create_dir_all(&bazel_external).unwrap();
        std::fs::create_dir_all(&cache_repo).unwrap();

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&cache_repo, bazel_external.join("rules_rust+0.69.0"))
                .unwrap();
            std::os::unix::fs::symlink(
                std::path::PathBuf::from("..")
                    .join("bazel-external")
                    .join("rules_rust+0.69.0"),
                external.join("rules_rust"),
            )
            .unwrap();
        }
        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_dir(
                &cache_repo,
                bazel_external.join("rules_rust+0.69.0"),
            )
            .unwrap();
            std::os::windows::fs::symlink_dir(
                std::path::PathBuf::from("..")
                    .join("bazel-external")
                    .join("rules_rust+0.69.0"),
                external.join("rules_rust"),
            )
            .unwrap();
        }

        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();
        let mut prefixes = BTreeSet::new();
        prefixes.insert("external".to_owned());

        let exec = ensure_execroot(&project_norm, &prefixes).unwrap();

        assert!(exec.as_path().join("external").is_dir());
        assert_eq!(
            std::fs::canonicalize(exec.as_path().join("external").join("rules_rust")).unwrap(),
            std::fs::canonicalize(&cache_repo).unwrap()
        );
        assert_eq!(
            std::fs::canonicalize(exec.as_path().join("external").join("rules_rust+0.69.0"))
                .unwrap(),
            std::fs::canonicalize(&cache_repo).unwrap()
        );
        assert_eq!(
            std::fs::canonicalize(external.join("rules_rust")).unwrap(),
            std::fs::canonicalize(&cache_repo).unwrap()
        );
    }

    #[test]
    fn identical_prefix_sets_share_execroot() {
        reset_cache_for_test();
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path();
        std::fs::create_dir(project.join("buck-out")).unwrap();
        std::fs::create_dir(project.join("external")).unwrap();
        let project_norm = AbsNormPathBuf::new(project.to_path_buf()).unwrap();

        let mut prefixes = BTreeSet::new();
        prefixes.insert("buck-out".to_owned());
        prefixes.insert("external".to_owned());

        let a = ensure_execroot(&project_norm, &prefixes).unwrap();
        let b = ensure_execroot(&project_norm, &prefixes).unwrap();
        assert_eq!(a, b);
    }
}
