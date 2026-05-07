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
//! Builds a small directory under `<project_root>/buck-out/v2/execroot/<digest>/`
//! containing only the top-level path components an action's declared
//! inputs and tools require. The action runs with that directory as
//! its `cwd`, so `read_dir(cwd)` returns exactly the prefixes the
//! action needs — matching Bazel's exec_root invariant without
//! sandbox staging.
//!
//! Replaces the global allowlist-filtered execroot from Phase 2.5
//! (`kuro_core::cells::ensure_execroot_layout`).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Mutex;

use kuro_core::fs::artifact_path_resolver::ArtifactFs;
use kuro_execute::execute::request::CommandExecutionInput;
use kuro_execute::execute::request::CommandExecutionRequest;
use kuro_fs::paths::abs_norm_path::AbsNormPath;
use kuro_fs::paths::abs_norm_path::AbsNormPathBuf;

/// Names that should always be available in the execroot regardless
/// of whether the action explicitly declared an input under them.
///
/// `buck-out` is needed because tool paths in command lines reference
/// `buck-out/v2/gen/...` directly (rules_rust runner, process wrapper,
/// rustc, etc.); without it the cwd-relative path can't resolve.
/// `external` is needed because the bzlmod apparent-name alias dir at
/// `<workspace>/external/<apparent>` is how kuro routes
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
    use std::hash::Hash;
    use std::hash::Hasher;
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
/// Path: `<project_root>/buck-out/v2/execroot/<digest>/`. Directory
/// is created lazily; symlinks point to the workspace's
/// `<project_root>/<prefix>/` directory. If a prefix doesn't exist as
/// a workspace directory it's silently skipped — actions reference
/// real paths only.
pub(crate) fn ensure_execroot(
    project_root: &AbsNormPath,
    prefixes: &BTreeSet<String>,
) -> Option<AbsNormPathBuf> {
    if prefixes.is_empty() {
        return None;
    }
    let digest = digest_prefixes(prefixes);
    let execroot_rel = format!("buck-out/v2/execroot/{digest}");
    let execroot_abs: PathBuf = project_root.as_path().join(&execroot_rel);

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
        match link.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                // Stale or wrong target — refresh.
                let _ = std::fs::remove_file(&link);
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
