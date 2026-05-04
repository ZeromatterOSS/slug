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
use kuro_common::invocation_paths::InvocationPaths;
use kuro_common::legacy_configs::configs::LegacyBuckConfig;
use kuro_error::BuckErrorContext;
use kuro_events::daemon_id::DaemonId;
use kuro_execute::digest_config::DigestConfig;
use kuro_execute::execute::blocking::BlockingExecutor;
use kuro_execute::materialize::materializer::MaterializationMethod;
use kuro_execute_impl::materializers::deferred::DeferredMaterializerConfigs;
use kuro_execute_impl::sqlite::incremental_state_db::INCREMENTAL_DB_SCHEMA_VERSION;
use kuro_execute_impl::sqlite::incremental_state_db::IncrementalDbState;
use kuro_execute_impl::sqlite::incremental_state_db::IncrementalStateSqliteDb;
use kuro_execute_impl::sqlite::materializer_db::MATERIALIZER_DB_SCHEMA_VERSION;
use kuro_execute_impl::sqlite::materializer_db::MaterializerState;
use kuro_execute_impl::sqlite::materializer_db::MaterializerStateSqliteDb;
use kuro_fs::fs_util;
use kuro_fs::paths::abs_norm_path::AbsNormPath;
use kuro_fs::paths::file_name::FileName;

use crate::daemon::server::BuckdServerInitPreferences;

#[derive(Allocative)]
pub struct DiskStateOptions {
    pub sqlite_materializer_state: bool,
    // In future, this will include the config for dep files on disk
}

impl DiskStateOptions {
    pub fn new(
        _root_config: &LegacyBuckConfig,
        materialization_method: MaterializationMethod,
    ) -> kuro_error::Result<Self> {
        // sqlite materializer state requires deferred materializer.
        let sqlite_materializer_state = matches!(
            materialization_method,
            MaterializationMethod::Deferred | MaterializationMethod::DeferredSkipFinalArtifacts
        );
        Ok(Self {
            sqlite_materializer_state,
        })
    }
}

fn sqlite_db_setup_metadata_and_versions(
    schema_version: String,
    deferred_materializer_config: Option<&DeferredMaterializerConfigs>,
    daemon_id: &DaemonId,
) -> kuro_error::Result<(HashMap<String, String>, HashMap<String, String>)> {
    let metadata = kuro_events::metadata::collect(&daemon_id);

    let mut versions = HashMap::from([("schema_version".to_owned(), schema_version)]);

    if let Some(config) = deferred_materializer_config {
        versions.insert(
            "defer_write_actions".to_owned(),
            config.defer_write_actions.to_string(),
        );
    }

    if let Some(hostname) = metadata.get("hostname") {
        versions.insert("hostname".to_owned(), hostname.to_owned());
    }

    Ok((metadata, versions))
}

pub(crate) async fn maybe_initialize_materializer_sqlite_db(
    options: &DiskStateOptions,
    paths: InvocationPaths,
    io_executor: Arc<dyn BlockingExecutor>,
    root_config: &LegacyBuckConfig,
    deferred_materializer_configs: &DeferredMaterializerConfigs,
    digest_config: DigestConfig,
    init_ctx: &BuckdServerInitPreferences,
    daemon_id: &DaemonId,
) -> kuro_error::Result<(Option<MaterializerStateSqliteDb>, Option<MaterializerState>)> {
    if !options.sqlite_materializer_state {
        // When sqlite materializer state is disabled, we should always delete the materializer state db.
        // Otherwise, artifacts in buck-out will diverge from the state stored in db.
        io_executor
            .execute_io_inline(|| {
                fs_util::remove_all(paths.materializer_state_path())
                    .map_err(kuro_error::Error::from)
            })
            .await?;
        return Ok((None, None));
    }

    let _ = root_config;
    let (metadata, versions) = sqlite_db_setup_metadata_and_versions(
        MATERIALIZER_DB_SCHEMA_VERSION.to_string(),
        Some(deferred_materializer_configs),
        daemon_id,
    )?;

    // Most things in the rest of `metadata` should go in the metadata sqlite table.
    // TODO(scottcao): Narrow down what metadata we need and and insert them into the
    // metadata table before a feature rollout.
    let (db, load_result) = MaterializerStateSqliteDb::initialize(
        paths.materializer_state_path(),
        versions,
        metadata,
        io_executor,
        digest_config,
        init_ctx.reject_materializer_state.as_ref(),
    )
    .await?;

    // We know path not found or version mismatch is normal, but some sqlite failures
    // are worth logging here. TODO(scottcao): Refine our error types and figure out what
    // errors to log
    let materializer_state = load_result.ok();
    Ok((Some(db), materializer_state))
}

pub(crate) async fn maybe_initialize_incremental_sqlite_db(
    paths: InvocationPaths,
    io_executor: Arc<dyn BlockingExecutor>,
    _root_config: &LegacyBuckConfig,
    daemon_id: &DaemonId,
) -> kuro_error::Result<IncrementalDbState> {
    let _ = paths.incremental_state_path();
    let (metadata, versions) = sqlite_db_setup_metadata_and_versions(
        INCREMENTAL_DB_SCHEMA_VERSION.to_string(),
        None,
        daemon_id,
    )?;

    let incremental_db_state = IncrementalStateSqliteDb::initialize(
        paths.incremental_state_path(),
        versions,
        metadata,
        io_executor,
        // TODO(minglunli): I'm not convinced we need reject_identity for incremental state. iiuc, this is only used by restarter
        // but incremental state isn't as widely used as materializer so we prob shouldn't restart daemon even if that's out of sync?
        None,
    )
    .await?;
    Ok(incremental_db_state)
}

// Once we start storing disk state in the cache directory, we need to make sure
// kuro always deletes the cache directory if the cache is disabled.
// Otherwise, buck-out state can diverge from the state of on-disk cache when
// cache is disabled, causing kuro to use stale cache when reading from the
// cache is re-enabled. One way this can happen is that someone can build on
// an older revision with a kuro that doesn't understand the cache directory
// in between 2 builds on newer revisions with kuro that reads from the cache
// (for ex., as a part of a bisect), then the state can become stale.
// There are 2 (not foolproof) mitigations planned:
// 1) Read from the logs what the last kuro invocation was and check that the
// last kuro supported on-disk state. If not, delete the disk state.
// 2) Start always deleting the cache directory now until we add support for disk
// state in kuro.
// The following implements mitigation #2 by always deleting disk state.

/// Recursively deletes all elements under `cache_dir_path`, except for known dirs
/// listed in `known_dir_names`.
pub(crate) fn delete_unknown_disk_state(
    cache_dir_path: &AbsNormPath,
    known_dir_names: &[&FileName],
) -> kuro_error::Result<()> {
    let res: kuro_error::Result<()> = try {
        if cache_dir_path.exists() {
            for entry in fs_util::read_dir(cache_dir_path)? {
                let entry = entry?;
                let filename = entry.file_name();
                let filename = filename
                    .to_str()
                    .buck_error_context("Filename is not UTF-8")
                    .and_then(FileName::new)?;

                // known_dir_names is always small, so this contains isn't expensive
                if !known_dir_names.contains(&filename) || !entry.path().is_dir() {
                    fs_util::remove_all(cache_dir_path.join(filename))?;
                }
            }
        }
    };

    res.with_buck_error_context(|| {
        format!(
            "deleting unrecognized caches in {} to prevent them from going stale",
            &cache_dir_path
        )
    })
}

#[cfg(test)]
mod tests {
    use kuro_core::fs::project::ProjectRootTemp;
    use kuro_core::fs::project_rel_path::ProjectRelativePath;
    use kuro_fs::paths::forward_rel_path::ForwardRelativePath;

    use super::*;

    #[test]
    fn test_delete_all_from_cache_dir() {
        let fs_temp = ProjectRootTemp::new().unwrap();
        let fs = fs_temp.path();
        let cache_dir_path = fs.resolve(ProjectRelativePath::unchecked_new("buck-out/v2/cache"));
        let materializer_state_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "materializer_state/db.sqlite",
        ));
        let command_hashes_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "command_hashes/db.sqlite",
        ));
        fs_util::create_dir_all(materializer_state_db.parent().unwrap()).unwrap();
        fs_util::write(&materializer_state_db, b"").unwrap();
        fs_util::create_dir_all(command_hashes_db.parent().unwrap()).unwrap();
        fs_util::write(&command_hashes_db, b"").unwrap();
        assert!(materializer_state_db.exists());
        assert!(command_hashes_db.exists());

        delete_unknown_disk_state(&cache_dir_path, &[]).unwrap();

        assert!(!materializer_state_db.exists());
        assert!(!command_hashes_db.exists());
    }

    #[test]
    fn test_delete_from_cache_dir_with_known_dirs() {
        let fs_temp = ProjectRootTemp::new().unwrap();
        let fs = fs_temp.path();
        let cache_dir_path = fs.resolve(ProjectRelativePath::unchecked_new("buck-out/v2/cache"));
        let materializer_state_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "materializer_state/db.sqlite",
        ));
        let command_hashes_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "command_hashes/db.sqlite",
        ));
        fs_util::create_dir_all(materializer_state_db.parent().unwrap()).unwrap();
        fs_util::write(&materializer_state_db, b"").unwrap();
        fs_util::create_dir_all(command_hashes_db.parent().unwrap()).unwrap();
        fs_util::write(&command_hashes_db, b"").unwrap();
        assert!(materializer_state_db.exists());
        assert!(command_hashes_db.exists());

        delete_unknown_disk_state(
            &cache_dir_path,
            &[FileName::unchecked_new("materializer_state")],
        )
        .unwrap();

        assert!(materializer_state_db.exists());
        assert!(!command_hashes_db.exists());
    }
}
