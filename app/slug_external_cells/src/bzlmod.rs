/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Bzlmod external cell implementation for `bazel_dep()` modules from BCR.
//!
//! Bzlmod cells are fetched from registries (like BCR) and cached at absolute
//! paths (e.g., ~/.cache/slug/registry/...). This module provides the
//! implementation that reads from those cached locations.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use cmp_any::PartialEqAny;
use compact_str::CompactString;
use dice::DiceComputations;
use slug_build_api::actions::artifact::get_artifact_fs::GetArtifactFs;
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
use slug_core::cells::external::BzlmodCellSetup;
use slug_core::cells::external::ExternalCellOrigin;
use slug_core::cells::name::CellName;
use slug_core::cells::paths::CellRelativePath;
use slug_core::cells::paths::CellRelativePathBuf;
use slug_execute::digest_config::DigestConfig;
use slug_execute::digest_config::HasDigestConfig;
use slug_execute::materialize::materializer::HasMaterializer;
use slug_execute::materialize::materializer::WriteRequest;
use slug_fs::paths::forward_rel_path::ForwardRelativePathBuf;

/// File operations delegate for bzlmod cells from registries.
///
/// This delegate reads files directly from the cache location where
/// the module was extracted after being fetched from BCR.
#[derive(allocative::Allocative)]
pub(crate) struct BzlmodFileOpsDelegate {
    /// The cell name.
    cell_name: CellName,
    /// The absolute path to the cached/extracted module source.
    source_path: PathBuf,
    /// Digest config for computing file digests.
    digest_config: DigestConfig,
}

impl BzlmodFileOpsDelegate {
    pub fn new(cell_name: CellName, source_path: PathBuf, digest_config: DigestConfig) -> Self {
        Self {
            cell_name,
            source_path,
            digest_config,
        }
    }

    fn resolve_path(&self, path: &CellRelativePath) -> PathBuf {
        self.source_path.join(path.as_str())
    }

    fn make_cell_path(&self, path: &CellRelativePath) -> Arc<CellPath> {
        Arc::new(CellPath::new(
            self.cell_name,
            CellRelativePathBuf::from(path.to_owned()),
        ))
    }
}

#[async_trait]
impl FileOpsDelegate for BzlmodFileOpsDelegate {
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
                    "Failed to get metadata for bzlmod file {:?}: {}",
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

/// Declare all source files from a bzlmod cell with the materializer.
///
/// This walks the source directory recursively and registers all files via
/// `declare_write` so they exist at `buck-out/v2/external_cells/bzlmod/...`
/// paths when actions try to use them as inputs.
async fn declare_all_source_artifacts(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: &BzlmodCellSetup,
    source_path: &std::path::Path,
) -> slug_error::Result<()> {
    let artifact_fs = ctx.get_artifact_fs().await?;
    let buck_out_resolver = artifact_fs.buck_out_path_resolver();

    // Walk the source directory recursively and collect all files
    let mut requests = Vec::new();
    let mut stack: Vec<PathBuf> = vec![source_path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let mut read_dir = match tokio::fs::read_dir(&dir).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to read bzlmod directory {:?}: {}",
                    dir,
                    e
                ));
            }
        };

        while let Some(entry) = read_dir.next_entry().await.map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Environment,
                "Failed to read directory entry: {}",
                e
            )
        })? {
            let file_type = entry.file_type().await.map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to get file type: {}",
                    e
                )
            })?;

            if file_type.is_dir() {
                stack.push(entry.path());
                continue;
            }

            if !file_type.is_file() {
                continue;
            }

            // Compute cell-relative path
            let abs_path = entry.path();
            let rel = abs_path.strip_prefix(source_path).map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to compute relative path: {}",
                    e
                )
            })?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let cell_rel = CellRelativePath::unchecked_new(&rel_str);

            let path = buck_out_resolver
                .resolve_external_cell_source(cell_rel, ExternalCellOrigin::Bzlmod(setup.clone()));

            let content = tokio::fs::read(&abs_path).await.map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to read bzlmod file {:?}: {}",
                    abs_path,
                    e
                )
            })?;

            // Preserve the executable bit from the source so shell scripts
            // (e.g. rules_python's build_data_writer.sh) remain invokable when
            // slug materialises them under buck-out/v2/external_cells/bzlmod.
            let metadata = entry.metadata().await.map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to stat bzlmod file {:?}: {}",
                    abs_path,
                    e
                )
            })?;
            #[cfg(unix)]
            let is_executable = {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            };
            #[cfg(not(unix))]
            let is_executable = {
                let _ = metadata;
                false
            };

            requests.push(WriteRequest {
                path,
                content,
                is_executable,
            });
        }
    }

    if !requests.is_empty() {
        let materializer = ctx.per_transaction_data().get_materializer();
        materializer
            .declare_write(Box::new(move || Ok(requests)))
            .await
            .map(|_| ())?;
    }

    Ok(())
}

/// Get the file ops delegate for a bzlmod cell.
pub(crate) async fn get_file_ops_delegate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: BzlmodCellSetup,
) -> slug_error::Result<Arc<dyn FileOpsDelegate>> {
    let digest_config = ctx.global_data().get_digest_config();
    let source_path = PathBuf::from(setup.source_path.as_ref());

    // Declare all source files with the materializer so they exist
    // at buck-out/v2/external_cells/bzlmod/... paths during action execution.
    declare_all_source_artifacts(ctx, cell_name, &setup, &source_path).await?;

    Ok(Arc::new(BzlmodFileOpsDelegate::new(
        cell_name,
        source_path,
        digest_config,
    )))
}

/// Copy bzlmod content from cache to destination.
///
/// This is used by the expand function to copy the cached module content
/// into the project's external directory.
pub(crate) async fn copy_to_destination(
    setup: &BzlmodCellSetup,
    dest_path: &std::path::Path,
) -> slug_error::Result<()> {
    let source_path = PathBuf::from(setup.source_path.as_ref());

    // Ensure destination parent exists
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Environment,
                "Failed to create directory {:?}: {}",
                parent,
                e
            )
        })?;
    }

    // Copy recursively
    copy_dir_recursive(&source_path, dest_path).await
}

/// Recursively copy a directory.
async fn copy_dir_recursive(
    src: &std::path::Path,
    dst: &std::path::Path,
) -> slug_error::Result<()> {
    tokio::fs::create_dir_all(dst).await.map_err(|e| {
        slug_error::slug_error!(
            slug_error::ErrorTag::Environment,
            "Failed to create directory {:?}: {}",
            dst,
            e
        )
    })?;

    let mut entries = tokio::fs::read_dir(src).await.map_err(|e| {
        slug_error::slug_error!(
            slug_error::ErrorTag::Environment,
            "Failed to read directory {:?}: {}",
            src,
            e
        )
    })?;

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        slug_error::slug_error!(
            slug_error::ErrorTag::Environment,
            "Failed to read directory entry: {}",
            e
        )
    })? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_type = entry.file_type().await.map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Environment,
                "Failed to get file type: {}",
                e
            )
        })?;

        if file_type.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else if file_type.is_symlink() {
            // Copy symlink
            let target = tokio::fs::read_link(&src_path).await.map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to read symlink {:?}: {}",
                    src_path,
                    e
                )
            })?;

            #[cfg(unix)]
            {
                tokio::fs::symlink(&target, &dst_path).await.map_err(|e| {
                    slug_error::slug_error!(
                        slug_error::ErrorTag::Environment,
                        "Failed to create symlink {:?}: {}",
                        dst_path,
                        e
                    )
                })?;
            }
            #[cfg(windows)]
            {
                // On Windows, determine if target is dir or file
                if target.is_dir() {
                    tokio::fs::symlink_dir(&target, &dst_path)
                        .await
                        .map_err(|e| {
                            slug_error::slug_error!(
                                slug_error::ErrorTag::Environment,
                                "Failed to create symlink {:?}: {}",
                                dst_path,
                                e
                            )
                        })?;
                } else {
                    tokio::fs::symlink_file(&target, &dst_path)
                        .await
                        .map_err(|e| {
                            slug_error::slug_error!(
                                slug_error::ErrorTag::Environment,
                                "Failed to create symlink {:?}: {}",
                                dst_path,
                                e
                            )
                        })?;
                }
            }
        } else {
            // Copy regular file
            tokio::fs::copy(&src_path, &dst_path).await.map_err(|e| {
                slug_error::slug_error!(
                    slug_error::ErrorTag::Environment,
                    "Failed to copy {:?} to {:?}: {}",
                    src_path,
                    dst_path,
                    e
                )
            })?;
        }
    }

    Ok(())
}
