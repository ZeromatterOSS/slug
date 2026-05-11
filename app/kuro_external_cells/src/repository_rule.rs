/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

//! Repository rule external cell implementation.
//!
//! Repository rule cells are created by repository rules like `http_archive`,
//! `git_repository`, etc. The content is materialized to a path (typically
//! `bazel-external/<name>`) during module resolution.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use cmp_any::PartialEqAny;
use compact_str::CompactString;
use dice::DiceComputations;
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
use kuro_core::cells::external::RepositoryRuleCellSetup;
use kuro_core::cells::name::CellName;
use kuro_core::cells::paths::CellRelativePath;
use kuro_core::cells::paths::CellRelativePathBuf;
use kuro_execute::digest_config::DigestConfig;
use kuro_execute::digest_config::HasDigestConfig;
use kuro_fs::paths::forward_rel_path::ForwardRelativePathBuf;

/// File operations delegate for repository rule cells.
///
/// This delegate reads files directly from the materialized repository path
/// (e.g., `bazel-external/<repo_name>`).
#[derive(allocative::Allocative)]
pub(crate) struct RepositoryRuleFileOpsDelegate {
    /// The cell name.
    cell_name: CellName,
    /// The absolute path to the materialized repository content.
    source_path: PathBuf,
    /// Digest config for computing file digests.
    digest_config: DigestConfig,
}

impl RepositoryRuleFileOpsDelegate {
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
impl FileOpsDelegate for RepositoryRuleFileOpsDelegate {
    async fn read_file_if_exists(
        &self,
        _ctx: &mut DiceComputations<'_>,
        path: &'async_trait CellRelativePath,
    ) -> kuro_error::Result<ReadFileProxy> {
        let abs_path = self.resolve_path(path);
        Ok(ReadFileProxy::new_with_captures(
            abs_path,
            |abs_path| async move {
                match tokio::fs::read_to_string(&abs_path).await {
                    Ok(contents) => Ok(Some(contents)),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                    Err(e) => Err(kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Environment,
                        "Failed to read repository rule file {:?}: {}",
                        abs_path,
                        e
                    )),
                }
            },
        ))
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
                "Failed to read repository rule directory {:?}: {}",
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
            let file_name = entry.file_name().into_string().map_err(|_| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
                    "Non-UTF8 filename in {:?}",
                    abs_path
                )
            })?;

            let entry_path = entry.path();
            let resolved = tokio::fs::metadata(&entry_path).await;
            let file_type = match resolved {
                Ok(md) if md.is_dir() => FileType::Directory,
                Ok(_) => FileType::File,
                Err(_) => {
                    let st = entry.file_type().await.map_err(|e| {
                        kuro_error::kuro_error!(
                            kuro_error::ErrorTag::Environment,
                            "Failed to get file type: {}",
                            e
                        )
                    })?;
                    if st.is_dir() {
                        FileType::Directory
                    } else if st.is_symlink() {
                        FileType::Symlink
                    } else {
                        FileType::File
                    }
                }
            };

            entries.push(RawDirEntry {
                file_name: CompactString::from(file_name),
                file_type,
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
                    "Failed to get metadata for repository rule file {:?}: {}",
                    abs_path,
                    e
                ));
            }
        };

        if metadata.is_dir() {
            Ok(Some(RawPathMetadata::Directory))
        } else if metadata.is_symlink() {
            if let Ok(target_metadata) = tokio::fs::metadata(&abs_path).await {
                if target_metadata.is_dir() {
                    return Ok(Some(RawPathMetadata::Directory));
                }
            }

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

            // Treat symlinks as external
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
                    "Failed to read repository rule file {:?}: {}",
                    abs_path,
                    e
                )
            })?;

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

/// Get the file ops delegate for a repository rule cell.
pub(crate) async fn get_file_ops_delegate(
    ctx: &mut DiceComputations<'_>,
    cell_name: CellName,
    setup: RepositoryRuleCellSetup,
) -> kuro_error::Result<Arc<dyn FileOpsDelegate>> {
    let digest_config = ctx.global_data().get_digest_config();
    let source_path = PathBuf::from(setup.source_path.as_ref());

    Ok(Arc::new(RepositoryRuleFileOpsDelegate::new(
        cell_name,
        source_path,
        digest_config,
    )))
}

/// Copy repository rule content to destination.
///
/// This is used by the expand function to copy the materialized repository content
/// into the project's external directory.
pub(crate) async fn copy_to_destination(
    setup: &RepositoryRuleCellSetup,
    dest_path: &std::path::Path,
) -> kuro_error::Result<()> {
    let source_path = PathBuf::from(setup.source_path.as_ref());
    copy_to_destination_impl(&source_path, dest_path).await
}

/// Copy content from source to destination.
///
/// This is a shared implementation used by both repository_rule and extension_repo.
pub(crate) async fn copy_to_destination_impl(
    source_path: &std::path::Path,
    dest_path: &std::path::Path,
) -> kuro_error::Result<()> {
    // Ensure destination parent exists
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Environment,
                "Failed to create directory {:?}: {}",
                parent,
                e
            )
        })?;
    }

    // Copy recursively
    copy_dir_recursive(source_path, dest_path).await
}

/// Recursively copy a directory.
async fn copy_dir_recursive(
    src: &std::path::Path,
    dst: &std::path::Path,
) -> kuro_error::Result<()> {
    tokio::fs::create_dir_all(dst).await.map_err(|e| {
        kuro_error::kuro_error!(
            kuro_error::ErrorTag::Environment,
            "Failed to create directory {:?}: {}",
            dst,
            e
        )
    })?;

    let mut entries = tokio::fs::read_dir(src).await.map_err(|e| {
        kuro_error::kuro_error!(
            kuro_error::ErrorTag::Environment,
            "Failed to read directory {:?}: {}",
            src,
            e
        )
    })?;

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        kuro_error::kuro_error!(
            kuro_error::ErrorTag::Environment,
            "Failed to read directory entry: {}",
            e
        )
    })? {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let file_type = entry.file_type().await.map_err(|e| {
            kuro_error::kuro_error!(
                kuro_error::ErrorTag::Environment,
                "Failed to get file type: {}",
                e
            )
        })?;

        if file_type.is_dir() {
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else if file_type.is_symlink() {
            // Copy symlink
            let target = tokio::fs::read_link(&src_path).await.map_err(|e| {
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
                    "Failed to read symlink {:?}: {}",
                    src_path,
                    e
                )
            })?;

            #[cfg(unix)]
            {
                tokio::fs::symlink(&target, &dst_path).await.map_err(|e| {
                    kuro_error::kuro_error!(
                        kuro_error::ErrorTag::Environment,
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
                            kuro_error::kuro_error!(
                                kuro_error::ErrorTag::Environment,
                                "Failed to create symlink {:?}: {}",
                                dst_path,
                                e
                            )
                        })?;
                } else {
                    tokio::fs::symlink_file(&target, &dst_path)
                        .await
                        .map_err(|e| {
                            kuro_error::kuro_error!(
                                kuro_error::ErrorTag::Environment,
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
                kuro_error::kuro_error!(
                    kuro_error::ErrorTag::Environment,
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
