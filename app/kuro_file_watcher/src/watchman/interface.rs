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
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use dice::DiceTransactionUpdater;
use kuro_common::file_ops::dice::FileChangeTracker;
use kuro_common::ignores::ignore_set::IgnoreSet;
use kuro_common::legacy_configs::configs::LegacyBuckConfig;
use kuro_core::cells::CellResolver;
use kuro_core::cells::name::CellName;
use kuro_core::fs::project_rel_path::ProjectRelativePath;
use kuro_error::BuckErrorContext;
use kuro_events::dispatch::span_async;
use kuro_fs::paths::abs_norm_path::AbsNormPath;
use kuro_util::process::async_background_command;
use tracing::debug;
use tracing::info;
use watchman_client::expr::Expr;
use watchman_client::prelude::Connector;
use watchman_client::prelude::FileType;

use crate::file_watcher::FileWatcher;
use crate::mergebase::Mergebase;
use crate::notify::is_reserved_output_path;
use crate::stats::FileWatcherStats;
use crate::watchman::core::SyncableQuery;
use crate::watchman::core::SyncableQueryProcessor;
use crate::watchman::core::WatchmanEvent;
use crate::watchman::core::WatchmanEventType;
use crate::watchman::core::WatchmanKind;
use crate::watchman::utils::find_first_valid_parent;

struct WatchmanQueryProcessor {
    // FIXME(JakobDegen): Storing these values statically is completely broken. See
    // `tests/e2e/cells/test_file_watcher_resolution:test_changing_cell_location_bug` for a repro of
    // a bug.
    cells: CellResolver,
    ignore_specs: HashMap<CellName, IgnoreSet>,
    empty_on_fresh_instance: bool,
    report_global_rev: bool,
    last_mergebase: Option<String>,
    last_mergebase_global_rev: Option<u64>,
    last_mergebase_timestamp: Option<u64>,
}

/// Used in process_one_change
#[derive(Debug)]
enum ChangeEvent<'a> {
    Watchman(&'a WatchmanEvent),
    SyntheticDirectoryChange,
}

impl WatchmanQueryProcessor {
    async fn process_events_impl(
        &self,
        mut ctx: DiceTransactionUpdater,
        events: Vec<WatchmanEvent>,
        base_stats: kuro_data::FileWatcherStats,
    ) -> kuro_error::Result<(kuro_data::FileWatcherStats, DiceTransactionUpdater)> {
        let mut handler = FileChangeTracker::new();

        let mut stats = FileWatcherStats::new(base_stats, events.len());

        for ev in events {
            // If the path is invalid, then walk up all the way until you find a valid dir to
            // invalidate listings. We don't need to invalidate the file itself, as we can't
            // read invalid files.

            let (path, event) = match ProjectRelativePath::new(&ev.path) {
                Ok(path) => (path, ChangeEvent::Watchman(&ev)),
                Err(_) => {
                    // If we error out here then we might miss other changes. This seems like
                    // it shouldn't happen, since the empty path should always be a valid path.
                    let path = find_first_valid_parent(&ev.path)
                        .with_buck_error_context(|| {
                            format!("Invalid path had no valid parent: `{}`", ev.path.display())
                        })
                        .unwrap();

                    (path, ChangeEvent::SyntheticDirectoryChange)
                }
            };

            self.process_one_change(path, event, &mut handler, &mut stats)?;
        }

        let stats = stats.finish();
        handler.write_to_dice(&mut ctx)?;

        Ok((stats, ctx))
    }

    fn process_one_change(
        &self,
        path: &ProjectRelativePath,
        ev: ChangeEvent<'_>,
        handler: &mut FileChangeTracker,
        stats: &mut FileWatcherStats,
    ) -> kuro_error::Result<()> {
        // Skip events under build-system output dirs (kuro's `buck-out` and bazel's
        // convenience symlinks). Mirrors the notify backend's filter so both backends
        // ignore the same noise.
        if is_reserved_output_path(path) {
            return Ok(());
        }

        let cell_path = self.cells.get_cell_path(path);

        let ignore = self
            .ignore_specs
            .get(&cell_path.cell())
            // This shouldn't ever really happen. However, because of the bugs caused by just
            // storing the `CellResolver` in the watcher permanently, sometimes it can, so we just
            // default to not ignoring the file in that case
            .is_some_and(|ignore| ignore.is_match(cell_path.path()));

        info!("Watchman: {:?} (ignore = {})", ev, ignore);

        if ignore {
            stats.add_ignored(1);
        } else {
            let cell_path_str = cell_path.to_string();
            let log_kind;
            let log_event;

            match ev {
                ChangeEvent::Watchman(ev) => match (&ev.kind, &ev.event) {
                    (WatchmanKind::File, typ) => {
                        log_kind = kuro_data::FileWatcherKind::File;
                        match typ {
                            WatchmanEventType::Modify => {
                                handler.file_contents_changed(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Modify;
                            }
                            WatchmanEventType::Create => {
                                handler.file_added_or_removed(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Create;
                            }
                            WatchmanEventType::Delete => {
                                handler.file_added_or_removed(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Delete;
                            }
                        }
                    }
                    (WatchmanKind::Directory, typ) => {
                        log_kind = kuro_data::FileWatcherKind::Directory;
                        match typ {
                            WatchmanEventType::Modify => {
                                handler.dir_entries_changed_for_watchman_bug(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Modify;
                            }
                            WatchmanEventType::Create => {
                                handler.dir_added_or_removed(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Create;
                            }
                            WatchmanEventType::Delete => {
                                handler.dir_added_or_removed(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Delete;
                            }
                        }
                    }
                    (WatchmanKind::Symlink, typ) => {
                        log_kind = kuro_data::FileWatcherKind::Symlink;
                        match typ {
                            WatchmanEventType::Modify => {
                                handler.file_contents_changed(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Modify;
                            }
                            WatchmanEventType::Create => {
                                debug!(
                                    "New symlink detected (source symlinks are not supported): {}",
                                    cell_path
                                );
                                handler.file_added_or_removed(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Create;
                            }
                            WatchmanEventType::Delete => {
                                handler.file_added_or_removed(cell_path);
                                log_event = kuro_data::FileWatcherEventType::Delete;
                            }
                        }
                    }
                },
                ChangeEvent::SyntheticDirectoryChange => {
                    log_kind = kuro_data::FileWatcherKind::Directory;
                    log_event = kuro_data::FileWatcherEventType::Modify;
                    // FIXME(JakobDegen): Add comment explaining why this is needed.
                    handler.dir_entries_changed_force_invalidate(cell_path);
                }
            };

            stats.add(cell_path_str, log_event, log_kind);
        }

        Ok(())
    }
}

struct RevisionDetails {
    global_rev: u64,
    timestamp: u64,
}

async fn try_fetch_revision_details(hash: &str) -> Option<RevisionDetails> {
    // There's a variety of ways in which this might go wrong: `PATH` is messed up, this somehow got
    // turned on in a non-`hg` repo, etc. To make sure we don't fail any builds from this, ignore
    // all errors.
    let command = async_background_command("hg")
        .args([
            "log",
            "-r",
            hash,
            "-T",
            "{get(extras, \"global_rev\")}\n{date}",
        ])
        .env("HPGPLAIN", "1")
        .output();
    let output = tokio::time::timeout(std::time::Duration::from_millis(500), command)
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let (global_rev_string, time_string) = stdout.trim().split_once('\n')?;
    Some(RevisionDetails {
        global_rev: global_rev_string.parse::<u64>().ok()?,
        timestamp: time_string.parse::<f64>().ok()? as u64, // hg returns the fractional seconds
    })
}

#[async_trait]
impl SyncableQueryProcessor for WatchmanQueryProcessor {
    type Output = kuro_data::FileWatcherStats;
    type Payload = DiceTransactionUpdater;

    async fn process_events(
        &mut self,
        dice: DiceTransactionUpdater,
        events: Vec<WatchmanEvent>,
        mergebase: &Option<String>,
        watchman_version: Option<String>,
    ) -> kuro_error::Result<(Self::Output, DiceTransactionUpdater)> {
        self.last_mergebase = mergebase.clone();
        self.process_events_impl(
            dice,
            events,
            kuro_data::FileWatcherStats {
                branched_from_revision: self.last_mergebase.clone(),
                branched_from_global_rev: self.last_mergebase_global_rev,
                branched_from_revision_timestamp: self.last_mergebase_timestamp,
                watchman_version,
                ..Default::default()
            },
        )
        .await
    }

    async fn on_fresh_instance(
        &mut self,
        ctx: DiceTransactionUpdater,
        events: Vec<WatchmanEvent>,
        mergebase: &Option<String>,
        watchman_version: Option<String>,
    ) -> kuro_error::Result<(Self::Output, DiceTransactionUpdater)> {
        let has_new_mergebase = self.last_mergebase.as_ref() != mergebase.as_ref();

        let clear_dep_files = has_new_mergebase;

        // We'll clear dep files if the mergebase has changed, which means our dep files are likely
        // irrelevant.
        //
        // This is imperfect. If the user rebased from yesterday's stable to today's stable, then
        // flushing is the right thing to do. In contrast, if they rebased from X to X's parent,
        // then it probably isn't. The consequences of flushing in the latter case aren't as bad
        // (where we'll rebuild things our dep files *could* have avoided) as not flushing in the
        // former (where we'll fetch loads of dep files that all miss), so we err on the side of
        // being safe and drop them when the mergebase changes.
        //
        // We do retain dep files that were produced locally, since we don't need to fetch them as
        // they are already on disk.
        if clear_dep_files {
            crate::dep_files::flush_non_local_dep_files();
        }

        self.last_mergebase = mergebase.clone();

        if let Some(hash) = self.last_mergebase.as_ref() {
            if self.report_global_rev {
                if let Some(revision_details) = try_fetch_revision_details(hash).await {
                    self.last_mergebase_global_rev = Some(revision_details.global_rev);
                    self.last_mergebase_timestamp = Some(revision_details.timestamp);
                } else {
                    self.last_mergebase_global_rev = None;
                    self.last_mergebase_timestamp = None;
                }
            }
        }

        // TODO(cjhopman): could probably get away with just invalidating all fs things, but that's not supported.
        // Dropping the entire DICE map can be somewhat computationally expensive as there
        // are a lot of destructors to run. On the other hand, we don't have to wait for
        // it. So, we just send it off to its own thread.
        let ctx = ctx.unstable_take();

        let mut base_stats = kuro_data::FileWatcherStats {
            fresh_instance: true,
            branched_from_revision: mergebase.clone(),
            branched_from_global_rev: self.last_mergebase_global_rev,
            branched_from_revision_timestamp: self.last_mergebase_timestamp,
            watchman_version,
            fresh_instance_data: Some(kuro_data::FreshInstance {
                new_mergebase: has_new_mergebase,
                cleared_dice: true,
                cleared_dep_files: clear_dep_files,
            }),
            ..Default::default()
        };

        if self.empty_on_fresh_instance {
            base_stats.incomplete_events_reason = Some("Fresh instance".to_owned());
            Ok((base_stats, ctx))
        } else {
            self.process_events_impl(ctx, events, base_stats).await
        }
    }
}

#[derive(Allocative)]
pub(crate) struct WatchmanFileWatcher {
    #[allocative(skip)]
    query: SyncableQuery<kuro_data::FileWatcherStats, DiceTransactionUpdater>,
}

/// The watchman query is constructed once on daemon startup. It is an unfiltered watchman query
/// over the entire project (though, like all watchman queries, still filtered by the
/// .watchmanconfig itself). Before any new computation request is started, it will be synced to
/// ensure that any recent changes are flushed and visible to the computation.
impl WatchmanFileWatcher {
    pub(crate) fn new(
        project_root: &AbsNormPath,
        root_config: &LegacyBuckConfig,
        cells: CellResolver,
        ignore_specs: HashMap<CellName, IgnoreSet>,
    ) -> kuro_error::Result<Self> {
        let _ = root_config;
        let watchman_merge_base: Option<String> = None;
        // When not using scm-aware queries, fresh instances would list every file in
        // the repo. That's a lot and not very useful.
        let empty_on_fresh_instance = true;
        let report_global_rev = false;

        let query = SyncableQuery::new(
            Connector::new(),
            project_root,
            Expr::Any(vec![
                Expr::FileType(FileType::Regular),
                Expr::FileType(FileType::Directory),
                Expr::FileType(FileType::Symlink),
            ]),
            Box::new(WatchmanQueryProcessor {
                cells,
                ignore_specs,
                empty_on_fresh_instance,
                report_global_rev,
                last_mergebase: None,
                last_mergebase_global_rev: None,
                last_mergebase_timestamp: None,
            }),
            watchman_merge_base,
            empty_on_fresh_instance,
        )?;

        Ok(Self { query })
    }
}

#[async_trait]
impl FileWatcher for WatchmanFileWatcher {
    async fn sync(
        &self,
        dice: DiceTransactionUpdater,
    ) -> kuro_error::Result<(DiceTransactionUpdater, Mergebase)> {
        span_async(
            kuro_data::FileWatcherStart {
                provider: kuro_data::FileWatcherProvider::Watchman as i32,
            },
            async {
                let (stats, res) = match self.query.sync(dice).await {
                    Ok((stats, dice)) => {
                        let mergebase = Mergebase(Arc::new(stats.branched_from_revision.clone()));
                        ((Some(stats)), Ok((dice, mergebase)))
                    }
                    Err(e) => (None, Err(e)),
                };
                (res, kuro_data::FileWatcherEnd { stats })
            },
        )
        .await
    }
}
