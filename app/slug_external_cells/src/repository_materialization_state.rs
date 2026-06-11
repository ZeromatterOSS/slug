/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory.
 */

use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine;
use dice::DiceComputations;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod::BzlmodEventKind;
use slug_bzlmod::RecordedDirtreeEntryState;
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
    let workspace_root = workspace_id.canonical_project_root.as_path();
    let dice_root = project_root.root().as_path();
    if !workspace_id.canonical_project_root.as_os_str().is_empty()
        && workspace_root != dice_root
        && !workspace_root.starts_with(dice_root)
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

fn repo_recorded_input_error(
    path: &ProjectRelativePath,
    reason: impl std::fmt::Display,
) -> Arc<str> {
    Arc::from(format!("failed to read recorded input '{}': {reason}", path).as_str())
}

async fn repo_recorded_file_marker_value_for_project_path(
    ctx: &mut DiceComputations<'_>,
    project_path: ProjectRelativePathBuf,
) -> Result<Arc<str>, Arc<str>> {
    record_bzlmod_event(
        BzlmodEventKind::RepoMaterializationStateRead,
        format!("metadata:{}", project_path.as_str()),
    );
    let metadata =
        DiceFileComputations::read_project_path_metadata_if_exists(ctx, project_path.as_ref())
            .await
            .map_err(|e| repo_recorded_input_error(project_path.as_ref(), e))?;
    match metadata {
        None => Ok(Arc::from("ENOENT")),
        Some(RawPathMetadata::Directory) => Ok(Arc::from("DIR")),
        Some(RawPathMetadata::File(_)) => {
            record_bzlmod_event(
                BzlmodEventKind::RepoMaterializationStateRead,
                format!("read_bytes:{}", project_path.as_str()),
            );
            let bytes =
                DiceFileComputations::read_project_file_bytes_if_exists(ctx, project_path.as_ref())
                    .await
                    .map_err(|e| repo_recorded_input_error(project_path.as_ref(), e))?
                    .ok_or_else(|| {
                        repo_recorded_input_error(project_path.as_ref(), "missing file")
                    })?;
            Ok(Arc::from(
                slug_bzlmod::compute_sha256_hex(bytes.as_slice()).as_str(),
            ))
        }
        Some(RawPathMetadata::Symlink { .. }) => Err(repo_recorded_input_error(
            project_path.as_ref(),
            "symlink recorded inputs are unsupported",
        )),
    }
}

async fn repo_recorded_dirents_marker_value_for_project_path(
    ctx: &mut DiceComputations<'_>,
    project_path: ProjectRelativePathBuf,
) -> Result<Arc<str>, Arc<str>> {
    record_bzlmod_event(
        BzlmodEventKind::RepoMaterializationStateRead,
        format!("dir_entries:{}", project_path.as_str()),
    );
    let entries = DiceFileComputations::read_project_dir_entry_names(ctx, project_path.as_ref())
        .await
        .map_err(|e| repo_recorded_input_error(project_path.as_ref(), e))?;
    Ok(Arc::from(
        slug_bzlmod::recorded_dirents_marker_value_from_entries(entries.as_ref()).as_str(),
    ))
}

fn repo_recorded_dirtree_marker_value_for_project_path<'a>(
    ctx: &'a mut DiceComputations<'_>,
    project_path: ProjectRelativePathBuf,
) -> Pin<Box<dyn Future<Output = Result<Arc<str>, Arc<str>>> + Send + 'a>> {
    Box::pin(async move {
        record_bzlmod_event(
            BzlmodEventKind::RepoMaterializationStateRead,
            format!("dir_entries:{}", project_path.as_str()),
        );
        let entries =
            DiceFileComputations::read_project_dir_entry_names(ctx, project_path.as_ref())
                .await
                .map_err(|e| repo_recorded_input_error(project_path.as_ref(), e))?;
        let mut entry_states = Vec::with_capacity(entries.len());
        for entry in entries.iter() {
            let child_path = child_project_path(&project_path, entry).ok_or_else(|| {
                repo_recorded_input_error(project_path.as_ref(), "invalid child path")
            })?;
            record_bzlmod_event(
                BzlmodEventKind::RepoMaterializationStateRead,
                format!("metadata:{}", child_path.as_str()),
            );
            let metadata = DiceFileComputations::read_project_path_metadata_if_exists(
                ctx,
                child_path.as_ref(),
            )
            .await
            .map_err(|e| repo_recorded_input_error(child_path.as_ref(), e))?
            .ok_or_else(|| repo_recorded_input_error(child_path.as_ref(), "missing path"))?;
            match metadata {
                RawPathMetadata::Directory => {
                    let digest =
                        repo_recorded_dirtree_marker_value_for_project_path(ctx, child_path)
                            .await?;
                    entry_states.push(RecordedDirtreeEntryState::DirectoryDigest(
                        digest.to_string(),
                    ));
                }
                RawPathMetadata::File(_) => {
                    record_bzlmod_event(
                        BzlmodEventKind::RepoMaterializationStateRead,
                        format!("read_bytes:{}", child_path.as_str()),
                    );
                    let bytes = DiceFileComputations::read_project_file_bytes_if_exists(
                        ctx,
                        child_path.as_ref(),
                    )
                    .await
                    .map_err(|e| repo_recorded_input_error(child_path.as_ref(), e))?
                    .ok_or_else(|| {
                        repo_recorded_input_error(child_path.as_ref(), "missing file")
                    })?;
                    entry_states.push(RecordedDirtreeEntryState::FileSha256(
                        Sha256::digest(bytes.as_slice()).to_vec(),
                    ));
                }
                RawPathMetadata::Symlink { .. } => {
                    entry_states.push(RecordedDirtreeEntryState::Other);
                }
            }
        }
        let digest = slug_bzlmod::recorded_dirtree_marker_value_from_entry_states(
            entries.as_ref(),
            &entry_states,
        )
        .map_err(|e| repo_recorded_input_error(project_path.as_ref(), e))?;
        Ok(Arc::from(digest.as_str()))
    })
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

    async fn repo_recorded_file_marker_value(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        recorded_path: Arc<PathBuf>,
    ) -> Result<Arc<str>, Arc<str>> {
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let project_path =
            repo_state_project_path(&workspace_id, project_root, recorded_path.as_ref())?;
        repo_recorded_file_marker_value_for_project_path(ctx, project_path).await
    }

    async fn repo_recorded_dirents_marker_value(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        recorded_path: Arc<PathBuf>,
    ) -> Result<Arc<str>, Arc<str>> {
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let project_path =
            repo_state_project_path(&workspace_id, project_root, recorded_path.as_ref())?;
        repo_recorded_dirents_marker_value_for_project_path(ctx, project_path).await
    }

    async fn repo_recorded_dirtree_marker_value(
        &self,
        ctx: &mut DiceComputations<'_>,
        workspace_id: WorkspaceId,
        recorded_path: Arc<PathBuf>,
    ) -> Result<Arc<str>, Arc<str>> {
        let io = ctx.global_data().get_io_provider();
        let project_root = io.project_root();
        let project_path =
            repo_state_project_path(&workspace_id, project_root, recorded_path.as_ref())?;
        repo_recorded_dirtree_marker_value_for_project_path(ctx, project_path).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dice::UserComputationData;
    use slug_common::dice::data::testing::SetTestingIoProvider;
    use slug_core::fs::project::ProjectRootTemp;

    use super::*;

    fn recorded_value(recorded: String) -> String {
        recorded
            .split_once(' ')
            .map(|(_, value)| value.to_owned())
            .expect("recorded input has value")
    }

    #[test]
    fn repo_state_project_path_accepts_nested_workspace_roots() {
        let fs = ProjectRootTemp::new().unwrap();
        let root = fs.path().root().as_path().to_path_buf();
        let nested_root = root.join("buck-out/external_cells/bzlmod/rules_rs/override");
        let state_path = nested_root.join("rs/extensions.bzl");
        let workspace_id =
            WorkspaceId::new(nested_root.clone(), nested_root.join("buck-out/custom"));

        let project_path = repo_state_project_path(&workspace_id, fs.path(), &state_path).unwrap();

        assert_eq!(
            project_path.as_str(),
            "buck-out/external_cells/bzlmod/rules_rs/override/rs/extensions.bzl"
        );
    }

    #[test]
    fn repo_state_project_path_rejects_foreign_workspace_roots() {
        let fs = ProjectRootTemp::new().unwrap();
        let root = fs.path().root().as_path().to_path_buf();
        let foreign_root = root
            .parent()
            .expect("temp project has parent")
            .join("foreign-workspace");
        let workspace_id = WorkspaceId::new(foreign_root.clone(), foreign_root.join("buck-out"));

        let err = repo_state_project_path(&workspace_id, fs.path(), &root.join("watched.txt"))
            .unwrap_err();

        assert_eq!(err.as_ref(), "repo_state_unreadable");
    }

    #[tokio::test]
    async fn recorded_input_markers_match_lockfile_format_through_dice_reads() {
        let fs = ProjectRootTemp::new().unwrap();
        let root = fs.path().root().as_path().to_path_buf();
        let watched = root.join("watched.txt");
        let dir = root.join("watched_dir");
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(&watched, "first\n").unwrap();
        std::fs::write(dir.join("a.txt"), "alpha\n").unwrap();
        std::fs::write(nested.join("b.txt"), "beta\n").unwrap();

        let workspace_id = WorkspaceId::new(root.clone(), root.join("buck-out/custom-output-base"));
        let mut dice = dice::testing::DiceBuilder::new()
            .set_data(|data| data.set_testing_io_provider(&fs))
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let file_marker = DICE_REPOSITORY_MATERIALIZATION_STATE_READER
            .repo_recorded_file_marker_value(
                &mut dice,
                workspace_id.clone(),
                Arc::new(watched.clone()),
            )
            .await
            .unwrap();
        assert_eq!(
            file_marker.as_ref(),
            recorded_value(slug_bzlmod::recorded_file_input(&watched).unwrap())
        );

        let dirents_marker = DICE_REPOSITORY_MATERIALIZATION_STATE_READER
            .repo_recorded_dirents_marker_value(
                &mut dice,
                workspace_id.clone(),
                Arc::new(dir.clone()),
            )
            .await
            .unwrap();
        assert_eq!(
            dirents_marker.as_ref(),
            recorded_value(slug_bzlmod::recorded_dirents_input(&dir).unwrap())
        );

        let dirtree_marker = DICE_REPOSITORY_MATERIALIZATION_STATE_READER
            .repo_recorded_dirtree_marker_value(
                &mut dice,
                workspace_id.clone(),
                Arc::new(dir.clone()),
            )
            .await
            .unwrap();
        assert_eq!(
            dirtree_marker.as_ref(),
            recorded_value(slug_bzlmod::recorded_dirtree_input(&dir).unwrap())
        );

        std::fs::write(&watched, "second\n").unwrap();

        // The Project* DICE keys are now cacheable (validity = x.is_ok()),
        // so an out-of-band file change requires explicit invalidation —
        // exactly as the file watcher does in the real daemon.
        let mut updater = dice.into_updater();
        let mut tracker = slug_common::file_ops::dice::FileChangeTracker::new();
        let watched_rel: slug_core::fs::project_rel_path::ProjectRelativePathBuf =
            watched.strip_prefix(&root).unwrap().to_path_buf().try_into().unwrap();
        tracker.project_file_contents_changed(watched_rel);
        tracker.write_to_dice(&mut updater).unwrap();
        let mut dice = updater.commit().await;

        let changed_file_marker = DICE_REPOSITORY_MATERIALIZATION_STATE_READER
            .repo_recorded_file_marker_value(&mut dice, workspace_id, Arc::new(watched))
            .await
            .unwrap();
        assert_ne!(file_marker, changed_file_marker);
    }
}
