/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

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
use slug_fs::paths::abs_path::AbsPath;

pub(crate) static DICE_REPOSITORY_MATERIALIZATION_STATE_READER:
    DiceRepositoryMaterializationStateReader = DiceRepositoryMaterializationStateReader;

pub(crate) struct DiceRepositoryMaterializationStateReader;

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

        if !workspace_id.canonical_project_root.as_os_str().is_empty()
            && workspace_id.canonical_project_root.as_path() != project_root.root().as_path()
        {
            tracing::debug!(
                workspace_root = %workspace_id.canonical_project_root.display(),
                dice_root = %project_root.root().display(),
                state_path = %state_path.display(),
                "repository materialization state root mismatch"
            );
            return Err(Arc::from("recorded_inputs_unreadable"));
        }

        let project_path = AbsPath::new(&state_path)
            .ok()
            .and_then(|path| project_root.relativize_any(path).ok())
            .ok_or_else(|| {
                tracing::debug!(
                    state_path = %state_path.display(),
                    project_root = %project_root.root().display(),
                    "repository materialization state path is not project-relative"
                );
                Arc::from("recorded_inputs_unreadable")
            })?;

        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationStateRead,
            project_path.as_str(),
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
                Arc::from("recorded_inputs_unreadable")
            })
    }
}
