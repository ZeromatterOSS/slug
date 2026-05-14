/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashMap;
use std::mem;
use std::sync::Arc;
use std::sync::Mutex;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceTransactionUpdater;
use dupe::Dupe;
use notify::EventKind;
use notify::RecommendedWatcher;
use notify::Watcher;
use notify::event::CreateKind;
use notify::event::MetadataKind;
use notify::event::ModifyKind;
use notify::event::RemoveKind;
use slug_common::file_ops::dice::FileChangeTracker;
use slug_common::ignores::ignore_set::IgnoreSet;
use slug_core::cells::CellResolver;
use slug_core::cells::cell_path::CellPath;
use slug_core::cells::name::CellName;
use slug_core::fs::project::ProjectRoot;
use slug_data::FileWatcherEventType;
use slug_data::FileWatcherKind;
use slug_error::conversion::from_any_with_tag;
use slug_events::dispatch::span_async;
use slug_fs::paths::abs_norm_path::AbsNormPath;
use starlark_map::ordered_set::OrderedSet;
use tracing::debug;
use tracing::info;

use crate::file_watcher::FileWatcher;
use crate::mergebase::Mergebase;
use crate::stats::FileWatcherStats;

/// Path components reserved for build-system output. Any project-relative path
/// containing one of these as a component is filtered out before reaching DICE
/// invalidation.
///
/// `buck-out` is slug's own output dir. The `bazel-*` entries are bazel's
/// convenience symlinks (`bazel-bin`, `bazel-out`, `bazel-testlogs`,
/// `bazel-bazel`, `bazel-external`). `install_filtered_watches` skips
/// symlinks at install time so we do not recurse into the trees they point
/// at, but some FS configurations still surface aliased events through
/// other watches; the component filter catches those.
const RESERVED_OUTPUT_COMPONENTS: &[&str] = &[
    "buck-out",
    "bazel-bin",
    "bazel-out",
    "bazel-testlogs",
    "bazel-bazel",
    "bazel-external",
    // <project_root>/execroot/<basename> is a self-symlink installed by
    // ensure_execroot_self_symlink in slug_core::cells. notify follows it
    // recursively into the project, so without this filter every change
    // also fires under `execroot/<basename>/...` and DICE invalidates
    // packages twice.
    "execroot",
];

pub(crate) fn is_reserved_output_path(
    path: &slug_core::fs::project_rel_path::ProjectRelativePath,
) -> bool {
    path.iter()
        .any(|c| RESERVED_OUTPUT_COMPONENTS.contains(&c.as_str()))
}

fn ignore_event_kind(event_kind: &EventKind) -> bool {
    match event_kind {
        EventKind::Access(_) => true,
        EventKind::Modify(ModifyKind::Metadata(MetadataKind::Ownership))
        | EventKind::Modify(ModifyKind::Metadata(MetadataKind::Permissions)) => false,
        EventKind::Modify(ModifyKind::Metadata(_)) => true,
        _ => false,
    }
}

/// Buffer containing the events that have happened since we last got a message.
/// Used to dedupe events, since notify sends a notification on every change.
#[derive(Allocative)]
struct NotifyFileData {
    ignored: u64,
    #[allocative(skip)]
    events: OrderedSet<(CellPath, EventKind)>,
    /// Whether file system changes were missed
    missed_events: bool,
}

impl NotifyFileData {
    fn new() -> Self {
        Self {
            ignored: 0,
            events: OrderedSet::new(),
            missed_events: false,
        }
    }

    fn process(
        &mut self,
        event: notify::Result<notify::Event>,
        root: &ProjectRoot,
        cells: &CellResolver,
        ignore_specs: &HashMap<CellName, IgnoreSet>,
    ) -> slug_error::Result<()> {
        let event = event.map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::NotifyWatcher))?;

        for path in &event.paths {
            // Testing shows that we get absolute paths back from the `notify` library.
            // It's not documented though.
            let path = root.relativize(AbsNormPath::new(&path)?)?;

            // Filter out events whose path includes a build-system output component (slug's
            // `buck-out` or any of bazel's convenience symlinks). When slug shares a workspace
            // with bazel — for instance the `@llvm-project//llvm:llvm` benchmark — every
            // bazel-written artifact under `bazel-bin/` would otherwise look like a source change
            // and force DICE re-evaluation on the next build, turning every warm-daemon rebuild
            // into a cold one.
            if is_reserved_output_path(&path) {
                continue;
            }

            let cell_path = cells.get_cell_path(&path);
            let ignore = ignore_specs
                .get(&cell_path.cell())
                // See the comment on the analogous code in `watchman/interface.rs`
                .is_some_and(|ignore| ignore.is_match(cell_path.path()));

            info!(
                "FileWatcher: {:?} {:?} (ignore = {})",
                path, &event.kind, ignore
            );

            if event.need_rescan() {
                self.missed_events = true;
                debug!("FileWatcher: File change events were missed");
            }

            if ignore || ignore_event_kind(&event.kind) {
                self.ignored += 1;
            } else {
                self.events.insert((cell_path, event.kind.clone()));
            }
        }
        Ok(())
    }

    fn sync(self) -> (slug_data::FileWatcherStats, Option<FileChangeTracker>) {
        // The changes that go into the DICE transaction
        let mut changed = FileChangeTracker::new();
        let mut stats = FileWatcherStats::new(Default::default(), self.events.len());
        stats.add_ignored(self.ignored);

        for (cell_path, event_kind) in self.events {
            let cell_path_str = cell_path.to_string();
            match event_kind {
                EventKind::Create(create_kind) => match create_kind {
                    CreateKind::File => {
                        changed.file_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                    }
                    CreateKind::Folder => {
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                    }
                    CreateKind::Any | CreateKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                EventKind::Modify(modify_kind) => match modify_kind {
                    ModifyKind::Data(_) | ModifyKind::Metadata(_) => {
                        changed.file_contents_changed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Modify,
                            FileWatcherKind::File,
                        );
                    }
                    ModifyKind::Name(_) | ModifyKind::Any | ModifyKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::File,
                        );
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Create,
                            FileWatcherKind::Directory,
                        );
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                EventKind::Remove(remove_kind) => match remove_kind {
                    RemoveKind::File => {
                        changed.file_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                    }
                    RemoveKind::Folder => {
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                    RemoveKind::Any | RemoveKind::Other => {
                        changed.file_added_or_removed(cell_path.clone());
                        stats.add(
                            cell_path_str.clone(),
                            FileWatcherEventType::Delete,
                            FileWatcherKind::File,
                        );
                        changed.dir_added_or_removed(cell_path);
                        stats.add(
                            cell_path_str,
                            FileWatcherEventType::Delete,
                            FileWatcherKind::Directory,
                        );
                    }
                },
                _ => {}
            }
        }

        let stats = stats.finish();
        let changed = if self.missed_events {
            None
        } else {
            Some(changed)
        };

        (stats, changed)
    }
}

/// Installs `notify` watches on every source directory under the project
/// root, filtering against the root cell's `[project].ignore` spec and
/// skipping symlinks so Bazel's `external/` convention (symlinks into
/// `bazel-external/`) doesn't pull thousands of generated-tree
/// directories into the inotify set.
///
/// Pre-fix, `NotifyFileWatcher::new` called
/// `watcher.watch(root, RecursiveMode::Recursive)` which made the
/// `notify` backend recursively walk every subdirectory and install an
/// inotify watch per directory — including the 29 000+ directories
/// inside `buck-out/` and the symlink-followed targets of `external/`.
/// That cold-start walk was measured at ~46 s on a fresh daemon with
/// the llvm-project workspace.
///
/// The post-fix walk uses `walkdir` with `follow_links(false)` and
/// installs a `RecursiveMode::NonRecursive` watch on each kept
/// directory. Newly-created directories surface as `Create` events on
/// the parent; the event processor can install watches for them on
/// demand (follow-up).
fn install_filtered_watches(
    watcher: &mut RecommendedWatcher,
    root: &ProjectRoot,
    cells: &CellResolver,
    ignore_specs: &HashMap<CellName, IgnoreSet>,
) -> slug_error::Result<()> {
    let root_path = root.root().as_path();
    let root_cell = cells.root_cell();
    let root_ignore = ignore_specs.get(&root_cell);

    let mut watched = 0usize;
    let mut skipped_ignored = 0usize;
    let mut skipped_symlink = 0usize;
    let walker = walkdir::WalkDir::new(root_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            // Always include the root itself so we can install a watch
            // on it for top-level event detection.
            if e.depth() == 0 {
                return true;
            }
            // Skip symlinks wholesale — on Bazel workspaces these point
            // into generated/external trees that aren't interesting for
            // source-change tracking.
            if e.file_type().is_symlink() {
                return false;
            }
            // Ignore-spec filter uses the relative path from the root.
            let rel = match e.path().strip_prefix(root_path) {
                Ok(r) => r,
                Err(_) => return true,
            };
            let Some(rel_str) = rel.to_str() else {
                return true;
            };
            if rel_str.is_empty() {
                return true;
            }
            let ignored = root_ignore.is_some_and(|ignores| {
                ignores.is_match(slug_core::cells::paths::CellRelativePath::unchecked_new(
                    rel_str,
                ))
            });
            !ignored
        });

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_dir() {
            // walkdir emits files too; only directories need a watch.
            // Count a symlink skip so the log reflects what we trimmed.
            if entry.file_type().is_symlink() {
                skipped_symlink += 1;
            }
            continue;
        }
        match watcher.watch(entry.path(), notify::RecursiveMode::NonRecursive) {
            Ok(()) => watched += 1,
            Err(e) => {
                // Filter-out errors come back as already-watched / path
                // vanished; log at debug to avoid noise.
                if watched < 5 {
                    tracing::warn!("notify: failed to watch {:?}: {}", entry.path(), e);
                }
                skipped_ignored += 1;
            }
        }
    }
    tracing::info!(
        "notify: installed watches on {} dirs, skipped {} symlinks / {} errors",
        watched,
        skipped_symlink,
        skipped_ignored,
    );
    Ok(())
}

#[derive(Allocative)]
pub struct NotifyFileWatcher {
    #[allocative(skip)]
    #[expect(unused)]
    // FIXME(JakobDegen): Clarify if this just needs to be kept alive or can be removed?
    watcher: RecommendedWatcher,
    data: Arc<Mutex<slug_error::Result<NotifyFileData>>>,
}

impl NotifyFileWatcher {
    pub fn new(
        root: &ProjectRoot,
        cells: CellResolver,
        ignore_specs: HashMap<CellName, IgnoreSet>,
    ) -> slug_error::Result<Self> {
        let data = Arc::new(Mutex::new(Ok(NotifyFileData::new())));
        let data2 = data.dupe();
        let root2 = root.dupe();
        let cells_for_cb = cells.clone();
        let ignore_specs_for_cb = ignore_specs.clone();
        let mut watcher = notify::recommended_watcher(move |event| {
            let mut guard = data2.lock().unwrap();
            if let Ok(state) = &mut *guard {
                if let Err(e) = state.process(event, &root2, &cells_for_cb, &ignore_specs_for_cb) {
                    *guard = Err(e);
                }
            }
        })
        .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::NotifyWatcher))?;

        install_filtered_watches(&mut watcher, root, &cells, &ignore_specs)?;

        Ok(Self { watcher, data })
    }

    fn sync2(
        &self,
        mut dice: DiceTransactionUpdater,
    ) -> slug_error::Result<(slug_data::FileWatcherStats, DiceTransactionUpdater)> {
        let mut guard = self.data.lock().unwrap();
        let old = mem::replace(&mut *guard, Ok(NotifyFileData::new()));
        let (stats, changes) = old?.sync();
        if let Some(changes) = changes {
            changes.write_to_dice(&mut dice)?;
        } else {
            // We missed some file system notifications, so we drop everything
            dice = dice.unstable_take();
        }
        Ok((stats, dice))
    }
}

#[async_trait]
impl FileWatcher for NotifyFileWatcher {
    async fn sync(
        &self,
        dice: DiceTransactionUpdater,
    ) -> slug_error::Result<(DiceTransactionUpdater, Mergebase)> {
        span_async(
            slug_data::FileWatcherStart {
                provider: slug_data::FileWatcherProvider::RustNotify as i32,
            },
            async {
                let (stats, res) = match self.sync2(dice) {
                    Ok((stats, dice)) => {
                        let mergebase = Mergebase(Arc::new(stats.branched_from_revision.clone()));
                        ((Some(stats)), Ok((dice, mergebase)))
                    }
                    Err(e) => (None, Err(e)),
                };
                (res, slug_data::FileWatcherEnd { stats })
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use slug_core::fs::project_rel_path::ProjectRelativePath;

    use super::is_reserved_output_path;

    fn check(path: &str) -> bool {
        is_reserved_output_path(ProjectRelativePath::unchecked_new(path))
    }

    #[test]
    fn buck_out_at_root() {
        assert!(check("buck-out/v2/foo"));
    }

    #[test]
    fn buck_out_nested() {
        assert!(check("external/cell/buck-out/foo"));
    }

    #[test]
    fn bazel_bin_at_root() {
        assert!(check("bazel-bin/llvm/llvm"));
    }

    #[test]
    fn bazel_external_nested() {
        assert!(check("bazel-external/+llvm_configure+/llvm/foo.cpp"));
    }

    #[test]
    fn similar_named_source_not_filtered() {
        assert!(!check("src/buckout-tool.rs"));
        assert!(!check("src/bazel-binutils/foo.rs"));
    }

    #[test]
    fn all_reserved_components_filtered() {
        for c in [
            "buck-out",
            "bazel-bin",
            "bazel-out",
            "bazel-testlogs",
            "bazel-bazel",
            "bazel-external",
        ] {
            assert!(check(&format!("{c}/foo")), "expected {c} to be filtered");
            assert!(
                check(&format!("a/b/{c}/c")),
                "expected nested {c} to be filtered"
            );
        }
    }
}
