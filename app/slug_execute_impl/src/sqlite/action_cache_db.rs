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

use chrono::Duration;
use chrono::Utc;
use dupe::Dupe;
use remote_execution::ActionResultResponse;
use slug_common::sqlite::sqlite_db::SqliteDb;
use slug_common::sqlite::sqlite_db::SqliteIdentity;
use slug_common::sqlite::sqlite_db::SqliteTable;
use slug_common::sqlite::sqlite_db::SqliteTables;
use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePath;
use slug_core::soft_error;
use slug_execute::execute::action_digest::ActionDigest;
use slug_execute::execute::blocking::BlockingExecutor;
use slug_fs::paths::abs_norm_path::AbsNormPath;
use slug_fs::paths::abs_norm_path::AbsNormPathBuf;

use crate::sqlite::tables::action_cache_table::ActionCacheSqliteTable;

/// Hand-maintained schema version for the action cache sqlite db.
/// PLEASE bump this version if you are making a breaking change to the
/// action cache sqlite db schema. If you forget to bump this version, then you
/// can fix forward by bumping the `slug.sqlite_action_cache_state_version`
/// buckconfig in the project root's .buckconfig.
pub const ACTION_CACHE_DB_SCHEMA_VERSION: u64 = 0;

/// Keep a margin below the common seven-day remote ActionCache/CAS TTL.
pub const DEFAULT_ACTION_CACHE_TTL_DAYS: u64 = 6;

pub struct ActionCacheDbState {
    db: Option<ActionCacheStateSqliteDb>,
    ttl: Duration,
}

impl ActionCacheDbState {
    pub fn db_disabled() -> Self {
        Self {
            db: None,
            ttl: default_action_cache_ttl(),
        }
    }

    pub fn get(&self, digest: &ActionDigest) -> Option<ActionResultResponse> {
        let db = self.db.as_ref()?;
        match db.action_cache_table().get(digest) {
            Ok(Some(entry)) => {
                let age_ms = Utc::now()
                    .timestamp_millis()
                    .saturating_sub(entry.cached_at_ms);
                if age_ms >= self.ttl.num_milliseconds() {
                    tracing::debug!(digest = %digest, "local action cache entry expired");
                    None
                } else {
                    Some(entry.action_result)
                }
            }
            Ok(None) => None,
            Err(e) => {
                soft_error!(
                    "read_from_action_cache_db",
                    slug_error::slug_error!(
                        slug_error::ErrorTag::Tier0,
                        "Failed to read action cache entry from sqlite db. {}",
                        e
                    ),
                    quiet: true
                )
                .unwrap();
                None
            }
        }
    }

    pub fn put(&self, digest: &ActionDigest, response: &ActionResultResponse) {
        let Some(db) = &self.db else {
            return;
        };

        if let Err(e) = db
            .action_cache_table()
            .put(digest, response, Utc::now().timestamp_millis())
        {
            soft_error!(
                "insert_to_action_cache_db",
                slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "Failed to insert action cache entry into sqlite db. {}",
                    e
                ),
                quiet: true
            )
            .unwrap();
        }
    }

    pub fn delete(&self, digest: &ActionDigest) {
        let Some(db) = &self.db else {
            return;
        };

        if let Err(e) = db.action_cache_table().delete(digest) {
            soft_error!(
                "delete_from_action_cache_db",
                slug_error::slug_error!(
                    slug_error::ErrorTag::Tier0,
                    "Failed to delete action cache entry from sqlite db. {}",
                    e
                ),
                quiet: true
            )
            .unwrap();
        }
    }
}

pub fn action_cache_ttl_from_days(days: u64) -> slug_error::Result<Duration> {
    Ok(Duration::days(days.try_into()?))
}

pub fn default_action_cache_ttl() -> Duration {
    action_cache_ttl_from_days(DEFAULT_ACTION_CACHE_TTL_DAYS)
        .expect("default action cache TTL should fit in chrono::Duration")
}

/// Concrete implementation of SqliteTable for ActionCacheSqliteTable.
impl SqliteTable for ActionCacheSqliteTable {
    fn create_table(&self) -> slug_error::Result<()> {
        ActionCacheSqliteTable::create_table(self)
    }
}

/// DB that opens the sqlite connection to the action cache state db on disk.
pub struct ActionCacheStateSqliteDb {
    tables: SqliteTables<ActionCacheSqliteTable>,
    /// A unique ID identifying this particular instance of the database.
    identity: SqliteIdentity,
}

impl SqliteDb for ActionCacheStateSqliteDb {
    type StateType = ();
    type TableType = ActionCacheSqliteTable;

    fn new(tables: SqliteTables<Self::TableType>) -> slug_error::Result<Self> {
        let identity = tables.get_identity()?;
        Ok(Self { tables, identity })
    }

    fn open_tables(path: &AbsNormPath) -> slug_error::Result<SqliteTables<Self::TableType>> {
        let connection = SqliteTables::<Self::TableType>::create_connection(path)?;
        let action_cache_table = ActionCacheSqliteTable::new(connection.dupe());
        Ok(SqliteTables::new(action_cache_table, connection))
    }

    fn identity(&self) -> &SqliteIdentity {
        &self.identity
    }
}

impl ActionCacheStateSqliteDb {
    pub async fn initialize(
        action_cache_state_dir: AbsNormPathBuf,
        versions: HashMap<String, String>,
        current_instance_metadata: HashMap<String, String>,
        io_executor: Arc<dyn BlockingExecutor>,
        ttl: Duration,
    ) -> slug_error::Result<ActionCacheDbState> {
        io_executor
            .execute_io_inline(|| {
                Self::initialize_action_cache_sqlite_db(
                    action_cache_state_dir,
                    versions,
                    current_instance_metadata,
                    ttl,
                )
            })
            .await
    }

    fn initialize_action_cache_sqlite_db(
        action_cache_state_dir: AbsNormPathBuf,
        versions: HashMap<String, String>,
        current_instance_metadata: HashMap<String, String>,
        ttl: Duration,
    ) -> slug_error::Result<ActionCacheDbState> {
        let db = match Self::get_sqlite_db(
            &action_cache_state_dir,
            &versions,
            current_instance_metadata.clone(),
            None,
        ) {
            Ok(db) => db,
            Err(e) => {
                tracing::debug!(
                    "Failed to load action cache sqlite state. Build will continue with a new empty local action cache. {}",
                    e
                );
                Self::create_sqlite_db(action_cache_state_dir, versions, current_instance_metadata)?
            }
        };

        Ok(ActionCacheDbState { db: Some(db), ttl })
    }

    pub(crate) fn action_cache_table(&self) -> &ActionCacheSqliteTable {
        &self.tables.domain_table
    }
}

#[allow(unused)] // Used by test modules.
pub(crate) fn testing_action_cache_sqlite_db(
    fs: &ProjectRoot,
    versions: HashMap<String, String>,
    metadata: HashMap<String, String>,
    ttl: Duration,
) -> slug_error::Result<ActionCacheDbState> {
    ActionCacheStateSqliteDb::initialize_action_cache_sqlite_db(
        fs.resolve(ProjectRelativePath::unchecked_new(
            "buck-out/v2/cache/action_cache_state",
        )),
        versions,
        metadata,
        ttl,
    )
}

#[cfg(test)]
mod tests {
    use remote_execution::ActionResultResponse;
    use remote_execution::TExecutedActionMetadata;
    use slug_common::cas_digest::CasDigestConfig;
    use slug_core::fs::project::ProjectRootTemp;
    use slug_execute::execute::action_digest::ActionDigest;

    use super::*;

    fn versions(version: u64) -> HashMap<String, String> {
        HashMap::from([("schema_version".to_owned(), version.to_string())])
    }

    fn metadata() -> HashMap<String, String> {
        HashMap::from([("created_by".to_owned(), "test_suite".to_owned())])
    }

    fn sample_digest() -> ActionDigest {
        ActionDigest::from_content(b"action", CasDigestConfig::testing_default())
    }

    fn sample_response(worker: &str) -> ActionResultResponse {
        ActionResultResponse {
            action_result: remote_execution::TActionResult2 {
                execution_metadata: TExecutedActionMetadata {
                    worker: worker.to_owned(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ttl: 0,
        }
    }

    #[test]
    fn state_get_put_roundtrip() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let state = testing_action_cache_sqlite_db(
            fs.path(),
            versions(ACTION_CACHE_DB_SCHEMA_VERSION),
            metadata(),
            default_action_cache_ttl(),
        )?;

        let digest = sample_digest();
        state.put(&digest, &sample_response("worker-1"));

        let response = state.get(&digest).expect("entry should be present");
        assert_eq!(response.action_result.execution_metadata.worker, "worker-1");

        Ok(())
    }

    #[test]
    fn state_delete_removes_entry() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let state = testing_action_cache_sqlite_db(
            fs.path(),
            versions(ACTION_CACHE_DB_SCHEMA_VERSION),
            metadata(),
            default_action_cache_ttl(),
        )?;

        let digest = sample_digest();
        state.put(&digest, &sample_response("worker-1"));
        assert!(state.get(&digest).is_some());

        state.delete(&digest);

        assert!(state.get(&digest).is_none());
        Ok(())
    }

    #[test]
    fn state_ignores_expired_entries() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let state = testing_action_cache_sqlite_db(
            fs.path(),
            versions(ACTION_CACHE_DB_SCHEMA_VERSION),
            metadata(),
            Duration::zero(),
        )?;

        let digest = sample_digest();
        state.put(&digest, &sample_response("worker-1"));

        assert!(state.get(&digest).is_none());

        Ok(())
    }

    #[test]
    fn version_mismatch_recreates_empty_db() -> slug_error::Result<()> {
        let fs = ProjectRootTemp::new()?;
        let state = testing_action_cache_sqlite_db(
            fs.path(),
            versions(0),
            metadata(),
            default_action_cache_ttl(),
        )?;

        let digest = sample_digest();
        state.put(&digest, &sample_response("worker-1"));
        assert!(state.get(&digest).is_some());

        let state = testing_action_cache_sqlite_db(
            fs.path(),
            versions(1),
            metadata(),
            default_action_cache_ttl(),
        )?;

        assert!(state.get(&digest).is_none());

        Ok(())
    }
}
