/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::borrow::Cow;
use std::sync::Arc;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use prost::Message;
use remote_execution::ActionResult;
use remote_execution::ActionResultResponse;
use rusqlite::Connection;
use slug_error::BuckErrorContext;
use slug_error::conversion::from_any_with_tag;
use slug_execute::execute::action_digest::ActionDigest;

const STATE_TABLE_NAME: &str = "action_cache";

#[derive(Debug)]
struct SqliteEntry<'a> {
    digest_hash: Cow<'a, [u8]>,
    digest_size: i64,
    action_result: Cow<'a, [u8]>,
    cached_at_ms: i64,
}

impl<'a> SqliteEntry<'a> {
    fn new(
        digest_hash: Cow<'a, [u8]>,
        digest_size: i64,
        action_result: Cow<'a, [u8]>,
        cached_at_ms: i64,
    ) -> Self {
        Self {
            digest_hash,
            digest_size,
            action_result,
            cached_at_ms,
        }
    }
}

fn digest_key(digest: &ActionDigest) -> slug_error::Result<(&[u8], i64)> {
    let digest_size = digest
        .size()
        .try_into()
        .buck_error_context("action digest size does not fit in sqlite INTEGER")?;
    Ok((digest.raw_digest().as_bytes(), digest_size))
}

fn encode_action_result(response: &ActionResultResponse) -> slug_error::Result<Vec<u8>> {
    let action_result = remote_execution::action_result_to_re_proto(response.action_result.clone())
        .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Tier0))
        .buck_error_context("converting action cache result to RE proto")?;
    Ok(action_result.encode_to_vec())
}

fn decode_action_result(data: &[u8]) -> slug_error::Result<ActionResultResponse> {
    let action_result = ActionResult::decode(data)
        .buck_error_context("decoding action cache result from sqlite")?;
    Ok(ActionResultResponse {
        action_result: remote_execution::action_result_from_re_proto(action_result)
            .map_err(|e| from_any_with_tag(e, slug_error::ErrorTag::Tier0))
            .buck_error_context("converting action cache result from RE proto")?,
        ttl: 0,
    })
}

pub(crate) struct ActionCacheSqliteEntry {
    pub action_result: ActionResultResponse,
    pub cached_at_ms: i64,
}

pub struct ActionCacheSqliteTable {
    connection: Arc<Mutex<Connection>>,
}

impl ActionCacheSqliteTable {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }

    pub(crate) fn create_table(&self) -> slug_error::Result<()> {
        let sql = format!(
            "CREATE TABLE {STATE_TABLE_NAME} (
                digest_hash            BLOB NOT NULL,
                digest_size            INTEGER NOT NULL,
                action_result          BLOB NOT NULL,
                cached_at_ms           INTEGER NOT NULL,
                PRIMARY KEY            (digest_hash, digest_size)
            )",
        );
        tracing::trace!(sql = %*sql, "creating table");
        self.connection
            .lock()
            .execute(&sql, [])
            .with_buck_error_context(|| format!("creating sqlite table {STATE_TABLE_NAME}"))?;

        let sql = format!(
            "CREATE INDEX {STATE_TABLE_NAME}_cached_at_ms ON {STATE_TABLE_NAME}(cached_at_ms)",
        );
        tracing::trace!(sql = %*sql, "creating index");
        self.connection
            .lock()
            .execute(&sql, [])
            .with_buck_error_context(|| {
                format!("creating sqlite index {STATE_TABLE_NAME}_cached_at_ms")
            })?;
        Ok(())
    }

    pub(crate) fn put(
        &self,
        digest: &ActionDigest,
        response: &ActionResultResponse,
        cached_at_ms: i64,
    ) -> slug_error::Result<()> {
        let (digest_hash, digest_size) = digest_key(digest)?;
        let encoded = encode_action_result(response)?;
        let entry = SqliteEntry::new(
            Cow::Borrowed(digest_hash),
            digest_size,
            Cow::Owned(encoded),
            cached_at_ms,
        );
        static SQL: Lazy<String> = Lazy::new(|| {
            format!(
                "INSERT OR REPLACE INTO {STATE_TABLE_NAME}
                    (digest_hash, digest_size, action_result, cached_at_ms)
                    VALUES (?1, ?2, ?3, ?4)"
            )
        });

        tracing::trace!(sql = %*SQL, entry = ?entry, "upserting action cache result");
        self.connection
            .lock()
            .execute(
                &SQL,
                rusqlite::params![
                    entry.digest_hash,
                    entry.digest_size,
                    entry.action_result,
                    entry.cached_at_ms,
                ],
            )
            .with_buck_error_context(|| {
                format!("upserting action result into sqlite table {STATE_TABLE_NAME}")
            })?;
        Ok(())
    }

    pub(crate) fn get(
        &self,
        digest: &ActionDigest,
    ) -> slug_error::Result<Option<ActionCacheSqliteEntry>> {
        let (digest_hash, digest_size) = digest_key(digest)?;
        static SQL: Lazy<String> = Lazy::new(|| {
            format!(
                "SELECT action_result, cached_at_ms FROM {STATE_TABLE_NAME}
                    WHERE digest_hash = ?1 AND digest_size = ?2"
            )
        });
        tracing::trace!(sql = %*SQL, digest = %digest, "reading action cache result");
        let connection = self.connection.lock();
        let mut stmt = connection.prepare(&SQL)?;
        let mut rows = stmt.query(rusqlite::params![digest_hash, digest_size])?;
        match rows.next()? {
            Some(row) => {
                let action_result: Vec<u8> = row.get(0)?;
                Ok(Some(ActionCacheSqliteEntry {
                    action_result: decode_action_result(&action_result)?,
                    cached_at_ms: row.get(1)?,
                }))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn delete(&self, digest: &ActionDigest) -> slug_error::Result<()> {
        let (digest_hash, digest_size) = digest_key(digest)?;
        static SQL: Lazy<String> = Lazy::new(|| {
            format!("DELETE FROM {STATE_TABLE_NAME} WHERE digest_hash = ?1 AND digest_size = ?2")
        });
        tracing::trace!(sql = %*SQL, digest = %digest, "deleting action cache result");
        self.connection
            .lock()
            .execute(&SQL, rusqlite::params![digest_hash, digest_size])
            .with_buck_error_context(|| {
                format!("deleting action result from sqlite table {STATE_TABLE_NAME}")
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use remote_execution::DigestWithStatus;
    use remote_execution::TAny;
    use remote_execution::TCode;
    use remote_execution::TDigest;
    use remote_execution::TExecutedActionMetadata;
    use remote_execution::TFile;
    use remote_execution::TStatus;
    use slug_common::cas_digest::CasDigestConfig;
    use slug_execute::execute::action_digest::ActionDigest;

    use super::*;

    fn sample_digest() -> ActionDigest {
        ActionDigest::from_content(b"action", CasDigestConfig::testing_default())
    }

    fn sample_response(worker: &str) -> ActionResultResponse {
        ActionResultResponse {
            action_result: remote_execution::TActionResult2 {
                output_files: vec![TFile {
                    digest: DigestWithStatus {
                        digest: TDigest {
                            hash: "abc123".to_owned(),
                            size_in_bytes: 6,
                            ..Default::default()
                        },
                        status: TStatus {
                            code: TCode::OK,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    name: "out.txt".to_owned(),
                    executable: true,
                    ..Default::default()
                }],
                stdout_raw: Some(b"stdout".to_vec()),
                stderr_raw: Some(b"stderr".to_vec()),
                execution_metadata: TExecutedActionMetadata {
                    worker: worker.to_owned(),
                    auxiliary_metadata: vec![TAny {
                        type_url: "type.example/cache".to_owned(),
                        value: b"metadata".to_vec(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                ..Default::default()
            },
            ttl: 0,
        }
    }

    #[test]
    fn put_get_roundtrip_preserves_action_result() -> slug_error::Result<()> {
        let connection = Connection::open_in_memory()?;
        let table = ActionCacheSqliteTable::new(Arc::new(Mutex::new(connection)));
        table.create_table()?;

        let digest = sample_digest();
        table.put(&digest, &sample_response("worker-1"), 123)?;

        let entry = table.get(&digest)?.expect("entry should be present");
        assert_eq!(entry.cached_at_ms, 123);
        assert_eq!(entry.action_result.action_result.exit_code, 0);
        assert_eq!(
            entry.action_result.action_result.stdout_raw.as_deref(),
            Some(&b"stdout"[..])
        );
        assert_eq!(
            entry.action_result.action_result.stderr_raw.as_deref(),
            Some(&b"stderr"[..])
        );
        assert_eq!(
            entry.action_result.action_result.execution_metadata.worker,
            "worker-1"
        );
        assert_eq!(
            entry
                .action_result
                .action_result
                .execution_metadata
                .auxiliary_metadata[0]
                .value,
            b"metadata"
        );

        Ok(())
    }

    #[test]
    fn put_replaces_existing_entry() -> slug_error::Result<()> {
        let connection = Connection::open_in_memory()?;
        let table = ActionCacheSqliteTable::new(Arc::new(Mutex::new(connection)));
        table.create_table()?;

        let digest = sample_digest();
        table.put(&digest, &sample_response("old-worker"), 123)?;
        table.put(&digest, &sample_response("new-worker"), 456)?;

        let entry = table.get(&digest)?.expect("entry should be present");
        assert_eq!(entry.cached_at_ms, 456);
        assert_eq!(
            entry.action_result.action_result.execution_metadata.worker,
            "new-worker"
        );

        Ok(())
    }

    #[test]
    fn delete_removes_entry() -> slug_error::Result<()> {
        let connection = Connection::open_in_memory()?;
        let table = ActionCacheSqliteTable::new(Arc::new(Mutex::new(connection)));
        table.create_table()?;

        let digest = sample_digest();
        table.put(&digest, &sample_response("worker-1"), 123)?;
        assert!(table.get(&digest)?.is_some());

        table.delete(&digest)?;

        assert!(table.get(&digest)?.is_none());
        Ok(())
    }
}
