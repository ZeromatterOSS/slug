/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

pub mod fs;
pub mod trace;

use allocative::Allocative;
use async_trait::async_trait;
use slug_core::cells::cell_path::CellPath;
use slug_core::fs::project::ProjectRoot;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_error::BuckErrorContext;
use slug_error::ErrorTag;

use crate::file_ops::metadata::RawDirEntry;
use crate::file_ops::metadata::RawPathMetadata;
use crate::ignores::file_ignores::FileIgnoreReason;

#[derive(Debug, Allocative, slug_error::Error)]
#[slug(tag = Input)]
pub enum ReadDirError {
    #[error("Directory `{path}` does not exist")]
    DirectoryDoesNotExist {
        path: CellPath,
        suggestion: DirectoryDoesNotExistSuggestion,
    },
    #[error("Directory `{0}` is ignored ({})", .1.describe())]
    DirectoryIsIgnored(CellPath, FileIgnoreReason),
    #[error("Path `{0}` is `{1}`, not a directory")]
    NotADirectory(CellPath, String),
    #[error(transparent)]
    Error(slug_error::Error),
}

#[derive(Debug, Allocative)]
pub enum DirectoryDoesNotExistSuggestion {
    Cell(Vec<String>),
    Typo(String),
    NoSuggestion,
}

impl From<slug_error::Error> for ReadDirError {
    fn from(value: slug_error::Error) -> Self {
        Self::Error(value)
    }
}

#[async_trait]
pub trait IoProvider: Allocative + Send + Sync {
    async fn read_file_if_exists_impl(
        &self,
        path: ProjectRelativePathBuf,
    ) -> slug_error::Result<Option<String>>;

    async fn read_dir_impl(
        &self,
        path: ProjectRelativePathBuf,
    ) -> slug_error::Result<Vec<RawDirEntry>>;

    async fn read_path_metadata_if_exists_impl(
        &self,
        path: ProjectRelativePathBuf,
    ) -> slug_error::Result<Option<RawPathMetadata<ProjectRelativePathBuf>>>;

    /// Request that this I/O provider be up to date with whatever I/O operations the user might
    /// have done until this point.
    async fn settle(&self) -> slug_error::Result<()>;

    fn name(&self) -> &'static str;

    /// Returns the Eden version of the underlying system of the IoProvider, if available.
    async fn eden_version(&self) -> slug_error::Result<Option<String>>;

    fn project_root(&self) -> &ProjectRoot;

    fn as_any(&self) -> &dyn std::any::Any;
}

impl dyn IoProvider + '_ {
    pub async fn read_file_if_exists(
        &self,
        path: ProjectRelativePathBuf,
    ) -> slug_error::Result<Option<String>> {
        self.read_file_if_exists_impl(path)
            .await
            .tag(ErrorTag::IoSource)
    }

    pub async fn read_dir(
        &self,
        path: ProjectRelativePathBuf,
    ) -> slug_error::Result<Vec<RawDirEntry>> {
        self.read_dir_impl(path).await.tag(ErrorTag::IoSource)
    }

    pub async fn read_path_metadata_if_exists(
        &self,
        path: ProjectRelativePathBuf,
    ) -> slug_error::Result<Option<RawPathMetadata<ProjectRelativePathBuf>>> {
        self.read_path_metadata_if_exists_impl(path)
            .await
            .tag(ErrorTag::IoSource)
    }
}
