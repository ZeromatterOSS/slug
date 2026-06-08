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
use cmp_any::PartialEqAny;
use compact_str::CompactString;
use dice::DiceComputations;
use dupe::Dupe;
use slug_common::dice::data::HasIoProvider;
use slug_common::external_symlink::ExternalSymlink;
use slug_common::file_ops::delegate::FileOpsDelegate;
use slug_common::file_ops::dice::ReadFileProxy;
use slug_common::file_ops::metadata::FileMetadata;
use slug_common::file_ops::metadata::FileType;
use slug_common::file_ops::metadata::RawDirEntry;
use slug_common::file_ops::metadata::RawPathMetadata;
use slug_common::file_ops::metadata::RawSymlink;
use slug_common::file_ops::metadata::TrackedFileDigest;
use slug_core::cells::cell_path::CellPath;
use slug_core::cells::external::LocalPathCellSetup;
use slug_core::cells::name::CellName;
use slug_core::cells::paths::CellRelativePath;
use slug_core::cells::paths::CellRelativePathBuf;
use slug_core::fs::project::ProjectRoot;
use slug_execute::digest_config::DigestConfig;
use slug_execute::digest_config::HasDigestConfig;
use slug_fs::paths::forward_rel_path::ForwardRelativePathBuf;

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
        ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> slug_error::Result<ReadFileProxy> {
        let abs_path = self.resolve_path(path);
        let value =
            slug_common::file_ops::dice::compute_watched_abs_file(ctx, abs_path).await?;
        let content = value.content.clone();
        Ok(ReadFileProxy::new_with_captures(content, |content| {
            let content = content.clone();
            async move {
                match content {
                    Some(bytes) => match String::from_utf8(bytes.to_vec()) {
                        Ok(s) => Ok(Some(s)),
                        Err(_) => Ok(None),
                    },
                    None => Ok(None),
                }
            }
        }))
    }

    async fn read_dir(
        &self,
        ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> slug_error::Result<Arc<[RawDirEntry]>> {
        let abs_path = self.resolve_path(path);
        let value =
            slug_common::file_ops::dice::compute_watched_abs_dir_entries(ctx, abs_path).await?;
        let entries: Arc<[RawDirEntry]> = value
            .entries
            .iter()
            .map(|e| RawDirEntry {
                file_name: CompactString::from(e.file_name.as_str()),
                file_type: e.file_type,
            })
            .collect();
        Ok(entries)
    }

    async fn read_path_metadata_if_exists(
        &self,
        ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> slug_error::Result<Option<RawPathMetadata>> {
        let abs_path = self.resolve_path(path);

        let meta_value = slug_common::file_ops::dice::compute_watched_abs_path_metadata(
            ctx,
            abs_path.clone(),
        )
        .await?;
        if !meta_value.exists {
            return Ok(None);
        }

        let metadata = match std::fs::symlink_metadata(&abs_path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to get metadata for {:?}: {}",
                    abs_path,
                    e
                ));
            }
        };

        if metadata.is_dir() {
            Ok(Some(RawPathMetadata::Directory))
        } else if metadata.is_symlink() {
            let target = std::fs::read_link(&abs_path).map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to read symlink {:?}: {}",
                    abs_path,
                    e
                )
            })?;

            let cell_path = self.make_cell_path(path);
            let external = ExternalSymlink::new(target, ForwardRelativePathBuf::empty())?;
            Ok(Some(RawPathMetadata::Symlink {
                at: cell_path,
                to: RawSymlink::External(Arc::new(external)),
            }))
        } else {
            let file_value = slug_common::file_ops::dice::compute_watched_abs_file(
                ctx,
                abs_path.clone(),
            )
            .await?;
            let contents = match &file_value.content {
                Some(bytes) => bytes,
                None => return Ok(None),
            };

            let source_config = self.digest_config.cas_digest_config().source_files_config();
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
) -> slug_error::Result<Arc<dyn FileOpsDelegate>> {
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
) -> slug_error::Result<slug_core::fs::project_rel_path::ProjectRelativePathBuf> {
    // Local path cells are already on the filesystem, so just return the path
    Ok(
        slug_core::fs::project_rel_path::ProjectRelativePathBuf::unchecked_new(
            setup.path.to_string(),
        ),
    )
}
