/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use dice::DiceComputations;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod::BzlmodEventKind;
use slug_bzlmod::RepositoryMaterializationStateReader;
use slug_bzlmod::WorkspaceId;
use slug_bzlmod::record_bzlmod_event;
use slug_common::dice::data::HasIoProvider;
use slug_common::file_ops::dice::DiceFileComputations;
use slug_common::file_ops::metadata::FileType;
use slug_common::file_ops::metadata::RawPathMetadata;
use slug_common::file_ops::metadata::RawSymlink;
use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePath;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_fs::paths::abs_path::AbsPath;

pub(crate) static DICE_REPOSITORY_MATERIALIZATION_STATE_READER:
    DiceRepositoryMaterializationStateReader = DiceRepositoryMaterializationStateReader;

pub(crate) struct DiceRepositoryMaterializationStateReader;

fn repo_state_project_path(
    workspace_id: &WorkspaceId,
    project_root: &ProjectRoot,
    state_path: &Path,
) -> Result<ProjectRelativePathBuf, Arc<str>> {
    if !workspace_id.canonical_project_root.as_os_str().is_empty()
        && workspace_id.canonical_project_root.as_path() != project_root.root().as_path()
    {
        tracing::debug!(
            workspace_root = %workspace_id.canonical_project_root.display(),
            dice_root = %project_root.root().display(),
            state_path = %state_path.display(),
            "repository materialization state root mismatch"
        );
        return Err(Arc::from("repo_state_unreadable"));
    }

    AbsPath::new(state_path)
        .ok()
        .and_then(|path| project_root.relativize_any(path).ok())
        .ok_or_else(|| {
            tracing::debug!(
                state_path = %state_path.display(),
                project_root = %project_root.root().display(),
                "repository materialization state path is not project-relative"
            );
            Arc::from("repo_state_unreadable")
        })
}

fn child_project_path(
    parent: &ProjectRelativePathBuf,
    child_name: &str,
) -> Option<ProjectRelativePathBuf> {
    let path = if parent.as_str().is_empty() {
        child_name.to_owned()
    } else {
        format!("{}/{}", parent.as_str(), child_name)
    };
    ProjectRelativePath::new(&path)
        .ok()
        .map(ProjectRelativePath::to_owned)
}

fn external_symlink_is_foreign(project_root: &ProjectRoot, target: &Path) -> bool {
    !target.starts_with(project_root.root().as_path())
}

fn symlink_target_matches_expected(
    project_root: &ProjectRoot,
    metadata: Option<RawPathMetadata<Arc<ProjectRelativePathBuf>>>,
    expected_target: &Path,
) -> bool {
    let Some(RawPathMetadata::Symlink { to, .. }) = metadata else {
        return false;
    };
    let actual_target = match to {
        RawSymlink::External(target) => target.to_path_buf(),
        RawSymlink::Relative(target, _) => project_root.root().as_path().join(target.as_str()),
    };
    actual_target == expected_target
}

fn hash_path_bytes(path: &Path, hasher: &mut Sha256) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hasher.update(path.as_os_str().as_bytes());
    }
    #[cfg(not(unix))]
    {
        hasher.update(path.to_string_lossy().as_bytes());
    }
}

fn repo_output_digest_error(
    path: &ProjectRelativePath,
    reason: impl std::fmt::Display,
) -> Arc<str> {
    Arc::from(format!("failed to digest repository output '{}': {reason}", path).as_str())
}

async fn hash_repository_output_entry(
    ctx: &mut DiceComputations<'_>,
    project_path: ProjectRelativePathBuf,
    relative_path: PathBuf,
    hasher: &mut Sha256,
) -> Result<(), Arc<str>> {
    let mut stack = vec![(project_path, relative_path)];
    while let Some((project_path, relative_path)) = stack.pop() {
        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationStateRead,
            format!("metadata:{}", project_path.as_str()),
        );
        let metadata =
            DiceFileComputations::read_project_path_metadata_if_exists(ctx, project_path.as_ref())
                .await
                .map_err(|e| repo_output_digest_error(project_path.as_ref(), e))?
                .ok_or_else(|| repo_output_digest_error(project_path.as_ref(), "missing path"))?;

        hash_path_bytes(&relative_path, hasher);
        match metadata {
            RawPathMetadata::Symlink { to, .. } => {
                hasher.update(b"L");
                match to {
                    RawSymlink::External(target) => {
                        hash_path_bytes(&target.to_path_buf(), hasher);
                    }
                    RawSymlink::Relative(_, target) => {
                        hash_path_bytes(Path::new(target.target().as_str()), hasher);
                    }
                }
            }
            RawPathMetadata::Directory => {
                hasher.update(b"D");
                record_bzlmod_event(
                    BzlmodEventKind::RepoMaterializationStateRead,
                    format!("dir_entries:{}", project_path.as_str()),
                );
                let entries =
                    DiceFileComputations::read_project_dir_entries(ctx, project_path.as_ref())
                        .await
                        .map_err(|e| repo_output_digest_error(project_path.as_ref(), e))?;
                for (name, _) in entries.iter().rev() {
                    if name == ".slug_repo_complete" || name == ".slug_repo_recorded_inputs" {
                        continue;
                    }
                    let child_project_path =
                        child_project_path(&project_path, name).ok_or_else(|| {
                            repo_output_digest_error(project_path.as_ref(), "invalid child path")
                        })?;
                    stack.push((child_project_path, relative_path.join(name)));
                }
            }
            RawPathMetadata::File(_) => {
                hasher.update(b"F");
                record_bzlmod_event(
                    BzlmodEventKind::RepoMaterializationStateRead,
                    format!("read_bytes:{}", project_path.as_str()),
                );
                let content = DiceFileComputations::read_project_file_bytes_if_exists(
                    ctx,
                    project_path.as_ref(),
                )
                .await
                .map_err(|e| repo_output_digest_error(project_path.as_ref(), e))?
                .ok_or_else(|| repo_output_digest_error(project_path.as_ref(), "missing file"))?;
                hasher.update((content.len() as u64).to_le_bytes());
                hasher.update(content.as_slice());
            }
        }
    }

    Ok(())
}

#[async_trait]
impl RepositoryMaterializationStateReader for DiceRepositoryMaterializationStateReader {
    async fn read_repo_state_file_if_exists(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        repo_dir: Arc<PathBuf>,
        file_name: &'static str,
    ) -> Result<Option<Arc<str>>, Arc<str>> {
        let state_path = repo_dir.join(file_name);
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let project_path = repo_state_project_path(&workspace_id, project_root, &state_path)?;

        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationStateRead,
            format!("read:{}", project_path.as_str()),
        );
        DiceFileComputations::read_project_file_if_exists(ctx, project_path.as_ref())
            .await
            .map(|content| content.map(|content| Arc::from(content.as_str())))
            .map_err(|e| {
                tracing::debug!(
                    error = %e,
                    state_path = %state_path.display(),
                    "failed to read repository materialization state through DICE"
                );
                Arc::from("repo_state_unreadable")
            })
    }

    async fn repo_state_file_exists(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        repo_dir: Arc<PathBuf>,
        file_name: &'static str,
    ) -> Result<bool, Arc<str>> {
        let state_path = repo_dir.join(file_name);
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let project_path = repo_state_project_path(&workspace_id, project_root, &state_path)?;

        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationStateRead,
            format!("metadata:{}", project_path.as_str()),
        );
        DiceFileComputations::read_project_path_metadata_if_exists(ctx, project_path.as_ref())
            .await
            .map(|metadata| metadata.is_some())
            .map_err(|e| {
                tracing::debug!(
                    error = %e,
                    state_path = %state_path.display(),
                    "failed to read repository materialization state metadata through DICE"
                );
                Arc::from("repo_state_unreadable")
            })
    }

    async fn repo_has_foreign_top_level_symlink(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        repo_dir: Arc<PathBuf>,
    ) -> Result<bool, Arc<str>> {
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let repo_project_path = repo_state_project_path(&workspace_id, project_root, &repo_dir)?;

        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationStateRead,
            format!("dir_entries:{}", repo_project_path.as_str()),
        );
        let entries =
            DiceFileComputations::read_project_dir_entries(ctx, repo_project_path.as_ref())
                .await
                .map_err(|e| {
                    tracing::debug!(
                        error = %e,
                        repo_dir = %repo_dir.display(),
                        "failed to read repository top-level entries through DICE"
                    );
                    Arc::from("repo_state_unreadable")
                })?;

        for (name, file_type) in entries.iter() {
            if *file_type != FileType::Symlink {
                continue;
            }
            let Some(path) = child_project_path(&repo_project_path, name) else {
                continue;
            };
            record_bzlmod_event(
                BzlmodEventKind::RepoMaterializationStateRead,
                format!("metadata:{}", path.as_str()),
            );
            let metadata =
                DiceFileComputations::read_project_path_metadata_if_exists(ctx, path.as_ref())
                    .await
                    .map_err(|e| {
                        tracing::debug!(
                            error = %e,
                            path = %path,
                            "failed to read repository top-level symlink metadata through DICE"
                        );
                        Arc::from("repo_state_unreadable")
                    })?;
            let Some(RawPathMetadata::Symlink {
                to: RawSymlink::External(target),
                ..
            }) = metadata
            else {
                continue;
            };
            if external_symlink_is_foreign(project_root, &target.to_path_buf()) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    async fn repo_dir_entry_names(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        dir: Arc<PathBuf>,
    ) -> Result<Arc<Vec<String>>, Arc<str>> {
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let project_path = repo_state_project_path(&workspace_id, project_root, &dir)?;

        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationStateRead,
            format!("dir_entries:{}", project_path.as_str()),
        );
        DiceFileComputations::read_project_dir_entry_names(ctx, project_path.as_ref())
            .await
            .map_err(|e| {
                tracing::debug!(
                    error = %e,
                    dir = %dir.display(),
                    "failed to read repository materialization directory entries through DICE"
                );
                Arc::from("repo_state_unreadable")
            })
    }

    async fn repo_symlink_points_to(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        symlink_path: Arc<PathBuf>,
        expected_target: Arc<PathBuf>,
    ) -> Result<bool, Arc<str>> {
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let project_path = repo_state_project_path(&workspace_id, project_root, &symlink_path)?;

        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationStateRead,
            format!("metadata:{}", project_path.as_str()),
        );
        let metadata =
            DiceFileComputations::read_project_path_metadata_if_exists(ctx, project_path.as_ref())
                .await
                .map_err(|e| {
                    tracing::debug!(
                        error = %e,
                        symlink_path = %symlink_path.display(),
                        "failed to read repository materialization symlink metadata through DICE"
                    );
                    Arc::from("repo_state_unreadable")
                })?;
        Ok(symlink_target_matches_expected(
            project_root,
            metadata,
            &expected_target,
        ))
    }

    async fn repo_output_digest(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        repo_dir: Arc<PathBuf>,
    ) -> Result<Arc<str>, Arc<str>> {
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let repo_project_path = repo_state_project_path(&workspace_id, project_root, &repo_dir)?;

        let mut hasher = Sha256::new();
        hash_repository_output_entry(ctx, repo_project_path, PathBuf::new(), &mut hasher).await?;
        let hash = hasher.finalize();
        Ok(Arc::from(
            format!(
                "sha256-{}",
                base64::engine::general_purpose::STANDARD.encode(hash)
            )
            .as_str(),
        ))
    }
}
