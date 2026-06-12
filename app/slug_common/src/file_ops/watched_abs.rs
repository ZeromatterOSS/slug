/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Registry + per-sync re-stat-diff for out-of-project (absolute-path) bzlmod
//! inputs (Plan 61 sub-plan 02 Phase A).
//!
//! The project file watcher proactively watches only the project root, so
//! out-of-project bzlmod inputs (local-path/git/archive override module files,
//! the out-of-project hidden lockfile) cannot be invalidated by it. Instead we
//! make them cacheable DICE inputs (`WatchedAbsFileKey` /
//! `WatchedAbsPathMetadataKey`) and inject their invalidation each command via a
//! re-stat-diff over this registry — the DICE-faithful analog of Bazel's
//! `ExternalDirtinessChecker` re-stat of `EXTERNAL_OTHER` files. This is a real
//! tracked DICE dependency, not an untracked read.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use allocative::Allocative;
use dice::DiceTransactionUpdater;
use sha2::Digest;
use sha2::Sha256;

use crate::file_ops::dice::FileChangeTracker;

/// Content digest of a file for change detection, or `None` if it does not exist.
/// Unreadable-but-present files hash their raw bytes so any byte change is seen.
fn current_file_digest(path: &Path) -> Option<String> {
    match std::fs::read(path) {
        Ok(bytes) => Some(hex::encode(Sha256::digest(&bytes))),
        Err(_) => None,
    }
}

#[derive(Default)]
struct WatchedAbsInputState {
    /// path -> last-seen content digest (`None` = absent/unreadable).
    files: HashMap<PathBuf, Option<String>>,
    /// path -> last-seen existence.
    paths: HashMap<PathBuf, bool>,
    /// Registered mutable-tree roots (e.g. bazel-external/… dirs). When a
    /// WatchedAbs key's path falls under one of these roots, the compute
    /// function depends on `ExternalTreeGenerationKey` so that
    /// re-materialization invalidates the cached leaf.
    tree_roots: Vec<PathBuf>,
    /// tree_root -> last-seen content digest of `.slug_repo_complete`.
    /// Used to detect re-materialization between per-sync re-stat-diffs so
    /// the generation node can be invalidated (which transitively invalidates
    /// all dependent WatchedAbs/Project* leaves).
    tree_generation_markers: HashMap<PathBuf, Option<String>>,
}

/// Daemon-owned set of registered out-of-project bzlmod input paths plus their
/// last-seen state. Lives in `DiceData` (so key computes can register paths) and
/// is held by the daemon (so the per-command sync can run the re-stat-diff). It is
/// NOT process-global mutable state — it is owned per daemon instance.
#[derive(Default, Allocative)]
pub struct WatchedAbsInputRegistry {
    #[allocative(skip)]
    inner: Mutex<WatchedAbsInputState>,
}

/// Paths whose on-disk state changed since the last re-stat-diff.
pub struct WatchedAbsChanges {
    pub files: Vec<PathBuf>,
    pub paths: Vec<PathBuf>,
    /// Mutable-tree roots whose `.slug_repo_complete` marker changed.
    pub tree_roots_changed: Vec<PathBuf>,
}

impl WatchedAbsChanges {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.paths.is_empty() && self.tree_roots_changed.is_empty()
    }
}

impl WatchedAbsInputRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `path`'s content is a tracked out-of-project bzlmod input. The
    /// supplied digest seeds the diff baseline on first registration (it is the
    /// value DICE just cached); subsequent updates are owned by `diff_and_update`.
    pub fn register_file(&self, path: PathBuf, current_digest: Option<String>) {
        if let Ok(mut state) = self.inner.lock() {
            state.files.entry(path).or_insert(current_digest);
        }
    }

    /// Record that `path`'s existence is a tracked out-of-project bzlmod input.
    pub fn register_path(&self, path: PathBuf, exists: bool) {
        if let Ok(mut state) = self.inner.lock() {
            state.paths.entry(path).or_insert(exists);
        }
    }

    /// Register `root` as a mutable-tree root (e.g. a bazel-external/… directory).
    /// When a WatchedAbs key's path falls under this root, its compute function
    /// will depend on `ExternalTreeGenerationKey` so that re-materialization
    /// invalidates the cached leaf.
    pub fn register_tree_root(&self, root: PathBuf) {
        if let Ok(mut state) = self.inner.lock() {
            if !state.tree_roots.iter().any(|r| r == &root) {
                let marker_digest = current_file_digest(&root.join(".slug_repo_complete"));
                state.tree_roots.push(root.clone());
                // Seed the marker baseline so the first diff_and_update
                // doesn't see a spurious change.
                state
                    .tree_generation_markers
                    .entry(root)
                    .or_insert(marker_digest);
            }
        }
    }

    /// If `path` starts with a registered mutable-tree root, return that root.
    /// Used by WatchedAbs compute functions to create a DICE dependency edge on
    /// `ExternalTreeGenerationKey` when the path is inside a mutable tree.
    pub fn is_under_tree_root(&self, path: &Path) -> Option<PathBuf> {
        let Ok(state) = self.inner.lock() else {
            return None;
        };
        for root in &state.tree_roots {
            if path.starts_with(root) {
                return Some(root.clone());
            }
        }
        None
    }

    /// Re-stat every registered path, return the ones that changed, and update the
    /// stored last-seen state. This is the `ExternalDirtinessChecker` analog.
    pub fn diff_and_update(&self) -> WatchedAbsChanges {
        let mut changes = WatchedAbsChanges {
            files: Vec::new(),
            paths: Vec::new(),
            tree_roots_changed: Vec::new(),
        };
        let Ok(mut state) = self.inner.lock() else {
            return changes;
        };
        for (path, last) in state.files.iter_mut() {
            let current = current_file_digest(path);
            if &current != last {
                changes.files.push(path.clone());
                *last = current;
            }
        }
        for (path, last) in state.paths.iter_mut() {
            let current = std::fs::symlink_metadata(path).is_ok();
            if current != *last {
                changes.paths.push(path.clone());
                *last = current;
            }
        }
        // Re-stat .slug_repo_complete for each registered tree root.
        let tree_roots = state.tree_roots.clone();
        for root in &tree_roots {
            let marker_path = root.join(".slug_repo_complete");
            let current = current_file_digest(&marker_path);
            let last = state.tree_generation_markers.get(root);
            if last != Some(&current) {
                changes.tree_roots_changed.push(root.clone());
                state.tree_generation_markers.insert(root.clone(), current);
            }
        }
        changes
    }
}

/// Run the re-stat-diff and inject `ctx.changed(...)` for every out-of-project
/// input whose on-disk state changed. Returns `true` if any change was injected
/// (so the caller can commit before recomputing config). Called from the
/// per-command sync alongside `FileWatcher::sync`.
pub fn inject_watched_abs_changes(
    registry: &WatchedAbsInputRegistry,
    updater: &mut DiceTransactionUpdater,
) -> slug_error::Result<bool> {
    let changes = registry.diff_and_update();
    if changes.is_empty() {
        return Ok(false);
    }
    let mut tracker = FileChangeTracker::new();
    for path in changes.files {
        tracker.abs_file_contents_changed(path);
    }
    for path in changes.paths {
        tracker.abs_path_added_or_removed(path);
    }
    for tree_root in changes.tree_roots_changed {
        tracker.ext_tree_generation_changed(tree_root);
    }
    tracker.write_to_dice(updater)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_detects_edit_create_delete_and_settles() {
        let dir = tempfile::Builder::new()
            .prefix("slug-plan61-watched-abs-registry-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let file = dir.path().join("MODULE.bazel");
        std::fs::write(&file, "a").unwrap();
        let meta = dir.path().join("override_dir_marker");

        let registry = WatchedAbsInputRegistry::new();
        registry.register_file(file.clone(), current_file_digest(&file));
        registry.register_path(meta.clone(), false);

        // No change yet.
        assert!(registry.diff_and_update().is_empty());

        // Edit the file -> detected once, then settles.
        std::fs::write(&file, "b").unwrap();
        let changes = registry.diff_and_update();
        assert_eq!(changes.files, vec![file.clone()]);
        assert!(registry.diff_and_update().is_empty());

        // Create the tracked path -> existence change detected.
        std::fs::write(&meta, "").unwrap();
        let changes = registry.diff_and_update();
        assert_eq!(changes.paths, vec![meta.clone()]);

        // Delete the file -> detected as a change (digest -> None).
        std::fs::remove_file(&file).unwrap();
        let changes = registry.diff_and_update();
        assert_eq!(changes.files, vec![file]);
    }

    #[test]
    fn diff_detects_generation_marker_change() {
        let dir = tempfile::Builder::new()
            .prefix("slug-plan63-gen-marker-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();

        // Create a tree root with an initial marker.
        let marker = dir.path().join(".slug_repo_complete");
        std::fs::write(&marker, "gen1").unwrap();

        let registry = WatchedAbsInputRegistry::new();
        registry.register_tree_root(dir.path().to_path_buf());

        // No change — marker seeded at registration.
        assert!(registry.diff_and_update().is_empty());

        // Change the marker -> generation change detected.
        std::fs::write(&marker, "gen2").unwrap();
        let changes = registry.diff_and_update();
        assert_eq!(changes.tree_roots_changed, vec![dir.path().to_path_buf()]);
        assert!(registry.diff_and_update().is_empty());

        // Delete the marker -> generation change detected (digest -> None).
        std::fs::remove_file(&marker).unwrap();
        let changes = registry.diff_and_update();
        assert_eq!(changes.tree_roots_changed, vec![dir.path().to_path_buf()]);
        assert!(registry.diff_and_update().is_empty());
    }
}
