/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Local path external cell implementation for bzlmod `local_path_override()`.
//!
//! Local path cells are already on the filesystem, so they don't need
//! materialization or special file operations. This module provides the
//! implementation that reads directly from the local filesystem.

use std::sync::Arc;

use async_trait::async_trait;
use kuro_common::dice::data::HasIoProvider;
use kuro_common::external_symlink::ExternalSymlink;
use kuro_common::file_ops::delegate::FileOpsDelegate;
use kuro_common::file_ops::dice::ReadFileProxy;
use kuro_common::file_ops::metadata::FileMetadata;
use kuro_common::file_ops::metadata::FileType;
use kuro_common::file_ops::metadata::RawDirEntry;
use kuro_common::file_ops::metadata::RawPathMetadata;
use kuro_common::file_ops::metadata::RawSymlink;
use kuro_common::file_ops::metadata::TrackedFileDigest;
use kuro_core::cells::cell_path::CellPath;
use kuro_core::cells::external::LocalPathCellSetup;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePath;
use kuro_core::cells::paths::CellRelativePathBuf;
use kuro_core::fs::project::ProjectRoot;
use kuro_execute::digest_config::DigestConfig;
use kuro_execute::digest_config::HasDigestConfig;
use kuro_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use cmp_any::PartialEqAny;
use compact_str::CompactString;
use dice::DiceComputations;
use dupe::Dupe;

/// File operations delegate for local path cells.
///
/// This delegate reads files directly from the local filesystem
/// at the path specified in the local_path_override().
#[derive(allocative::Allocative)]
pub(crate) struct LocalPathFileOpsDelegate {
    /// The project root for resolving paths.
    project_root: ProjectRoot,
    /// The cell name.
    cell_name: CellName,
    /// The path relative to project root where this cell lives.
    cell_path: String,
    /// Digest config for computing file digests.
    digest_config: DigestConfig,
}

impl LocalPathFileOpsDelegate {
    pub fn new(
        project_root: ProjectRoot,
        cell_name: CellName,
        cell_path: String,
        digest_config: DigestConfig,
    ) -> Self {
        Self {
            project_root,
            cell_name,
            cell_path,
            digest_config,
        }
    }

    fn resolve_path(&self, path: &CellRelativePath) -> std::path::PathBuf {
        self.project_root
            .root()
            .as_path()
            .join(&self.cell_path)
            .join(path.as_str())
    }

    fn make_cell_path(&self, path: &CellRelativePath) -> Arc<CellPath> {
        Arc::new(CellPath::new(
            self.cell_name,
            CellRelativePathBuf::from(path.to_owned()),
        ))
    }
}

#[async_trait]
impl FileOpsDelegate for LocalPathFileOpsDelegate {
    async fn read_file_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> kuro_error::Result<ReadFileProxy> {
        let abs_path = self.resolve_path(path);
        Ok(ReadFileProxy::new_with_captures(abs_path, |abs_path| async move {
            match tokio::fs::read_to_string(&abs_path).await {
                Ok(contents) => Ok(Some(contents)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
                    "Failed to read file {:?}: {}",
                    abs_path,
                    e
                )),
            }
        }))
    }

    async fn read_dir(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> kuro_error::Result<Arc<[RawDirEntry]>> {
        let abs_path = self.resolve_path(path);

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&abs_path).await.map_err(|e| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Environment,
                "Failed to read directory {:?}: {}",
                abs_path,
                e
            )
        })?;

        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Environment,
                "Failed to read directory entry: {}",
                e
            )
        })? {
            let file_type = entry.file_type().await.map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
                    "Failed to get file type: {}",
                    e
                )
            })?;

            let file_name = entry.file_name().into_string().map_err(|_| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
                    "Non-UTF8 filename in {:?}",
                    abs_path
                )
            })?;

            entries.push(RawDirEntry {
                file_name: CompactString::from(file_name),
                file_type: if file_type.is_dir() {
                    FileType::Directory
                } else if file_type.is_symlink() {
                    FileType::Symlink
                } else {
                    FileType::File
                },
            });
        }

        // Sort entries for deterministic output
        entries.sort_by(|a, b| a.file_name.cmp(&b.file_name));

        Ok(entries.into())
    }

    async fn read_path_metadata_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> kuro_error::Result<Option<RawPathMetadata>> {
        let abs_path = self.resolve_path(path);

        let metadata = match tokio::fs::symlink_metadata(&abs_path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
                    "Failed to get metadata for {:?}: {}",
                    abs_path,
                    e
                ))
            }
        };

        if metadata.is_dir() {
            Ok(Some(RawPathMetadata::Directory))
        } else if metadata.is_symlink() {
            // Read symlink target
            let target = tokio::fs::read_link(&abs_path).await.map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
                    "Failed to read symlink {:?}: {}",
                    abs_path,
                    e
                )
            })?;

            let cell_path = self.make_cell_path(path);

            // For local path cells, treat symlinks as external
            // (pointing outside the cell's scope)
            let external = ExternalSymlink::new(target, ForwardRelativePathBuf::empty())?;
            Ok(Some(RawPathMetadata::Symlink {
                at: cell_path,
                to: RawSymlink::External(Arc::new(external)),
            }))
        } else {
            // Regular file
            let contents = tokio::fs::read(&abs_path).await.map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
                    "Failed to read file {:?}: {}",
                    abs_path,
                    e
                )
            })?;

            let source_config = self
                .digest_config
                .cas_digest_config()
                .source_files_config();
            let digest = TrackedFileDigest::from_content(&contents, source_config);

            #[cfg(unix)]
            let is_executable = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let is_executable = false;

            Ok(Some(RawPathMetadata::File(FileMetadata {
                digest,
                is_executable,
            })))
        }
    }

    fn eq_token(&self) -> PartialEqAny<'_> {
        PartialEqAny::always_false()
    }
}

/// Get the file ops delegate for a local path cell.
pub(crate) async fn get_file_ops_delegate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: LocalPathCellSetup,
) -> kuro_error::Result<Arc<dyn FileOpsDelegate>> {
    let io = ctx.global_data().get_io_provider();
    let project_root = io.project_root().dupe();
    let digest_config = ctx.global_data().get_digest_config();

    Ok(Arc::new(LocalPathFileOpsDelegate::new(
        project_root,
        cell_name,
        setup.path.to_string(),
        digest_config,
    )))
}

/// For local path cells, materialization is a no-op since files already exist.
pub(crate) async fn materialize_all(
    _ctx: &mut DiceComputations<'_>,
    _cell: CellName,
    setup: LocalPathCellSetup,
) -> kuro_error::Result<kuro_core::fs::project_rel_path::ProjectRelativePathBuf> {
    // Local path cells are already on the filesystem, so just return the path
    Ok(kuro_core::fs::project_rel_path::ProjectRelativePathBuf::unchecked_new(
        setup.path.to_string(),
    ))
}
