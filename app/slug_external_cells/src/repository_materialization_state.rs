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
use dice::DiceComputations;
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
    ProjectRelativePath::new(&format!("{}/{}", parent.as_str(), child_name))
        .ok()
        .map(ProjectRelativePath::to_owned)
}

fn external_symlink_is_foreign(project_root: &ProjectRoot, target: &Path) -> bool {
    !target.starts_with(project_root.root().as_path())
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
}
