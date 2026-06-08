/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use allocative::Allocative;
use async_trait::async_trait;
use derive_more::Display;
use dice::DiceComputations;
use dice::DiceTransactionUpdater;
use dice::InvalidationSourcePriority;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::future::BoxFuture;
use sha2::Digest;
use sha2::Sha256;
use slug_core::cells::cell_path::CellPath;
use slug_core::cells::cell_path::CellPathRef;
use slug_core::cells::name::CellName;
use slug_core::fs::project_rel_path::ProjectRelativePath;
use slug_core::fs::project_rel_path::ProjectRelativePathBuf;
use slug_fs::paths::file_name::FileNameBuf;

use crate::buildfiles::HasBuildfiles;
use crate::dice::data::HasIoProvider;
use crate::dice::data::HasWatchedAbsInputRegistry;
use crate::file_ops::delegate::get_delegated_file_ops;
use crate::file_ops::error::FileReadError;
use crate::file_ops::error::extended_ignore_error;
use crate::file_ops::metadata::FileType;
use crate::file_ops::metadata::RawPathMetadata;
use crate::file_ops::metadata::ReadDirOutput;
use crate::file_ops::metadata::SimpleDirEntry;
use crate::ignores::file_ignores::FileIgnoreResult;
use crate::io::ReadDirError;

pub struct DiceFileComputations;

// Project-root bzlmod bootstrap inputs are read before the current command's
// cell resolver exists. Register exact project paths here so later watcher
// events can dirty their `ProjectReadFileKey`s before config recomputation.
static BZLMOD_CONFIG_PROJECT_FILE_INPUTS: std::sync::LazyLock<
    std::sync::RwLock<HashSet<ProjectRelativePathBuf>>,
> = std::sync::LazyLock::new(|| std::sync::RwLock::new(HashSet::new()));

pub fn register_bzlmod_config_project_file(path: ProjectRelativePathBuf) {
    if let Ok(mut paths) = BZLMOD_CONFIG_PROJECT_FILE_INPUTS.write() {
        paths.insert(path);
    }
}

/// Functions for accessing files with keys on the dice graph.
impl DiceFileComputations {
    /// Filters out ignored paths
    pub async fn read_dir(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> slug_error::Result<ReadDirOutput> {
        ctx.compute(&ReadDirKey {
            path: path.to_owned(),
            check_ignores: CheckIgnores::Yes,
        })
        .await?
    }

    /// Returns if a directory or file exists at the given path, but checks for an exact,
    /// case-sensitive match.
    ///
    /// Note that case-sensitive match is only done on the last element of the path, not any of the
    /// elements before.
    pub async fn exists_matching_exact_case(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> slug_error::Result<bool> {
        ctx.compute(&ExistsMatchingExactCaseKey(path.to_owned()))
            .await?
    }

    pub async fn read_dir_include_ignores(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> slug_error::Result<ReadDirOutput> {
        ctx.compute(&ReadDirKey {
            path: path.to_owned(),
            check_ignores: CheckIgnores::No,
        })
        .await?
    }

    /// Like read_dir, but with extended error information. This may add additional dice dependencies.
    pub async fn read_dir_ext(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> Result<ReadDirOutput, ReadDirError> {
        read_dir_ext(ctx, path).await
    }

    /// Does not check if the path is ignored
    ///
    /// TODO(cjhopman): error on ignored paths, maybe.
    pub async fn read_file_if_exists(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> slug_error::Result<Option<String>> {
        (ctx.compute(&ReadFileKey(Arc::new(path.to_owned())))
            .await??
            .0)()
        .await
    }

    /// Does not check if the path is ignored
    pub async fn read_file(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> Result<String, FileReadError> {
        match Self::read_file_if_exists(ctx, path).await {
            Ok(result) => result.ok_or_else(|| FileReadError::NotFound(path.to_string())),
            Err(e) => Err(FileReadError::Buck(e)),
        }
    }

    /// Read an out-of-project (absolute-path) bzlmod input file through a cacheable
    /// DICE input. Invalidation of the returned value is driven by the per-sync
    /// re-stat-diff over the daemon-owned registered-input registry (Plan 61 sub-plan
    /// 02). Callers must register the path with that registry so edits are observed.
    pub async fn read_watched_abs_file_if_exists(
        ctx: &mut DiceComputations<'_>,
        path: &Path,
    ) -> slug_error::Result<Option<String>> {
        let value = ctx
            .compute(&WatchedAbsFileKey {
                path: Arc::new(path.to_path_buf()),
            })
            .await??;
        match &value.content {
            Some(bytes) => {
                let s = String::from_utf8(bytes.to_vec()).map_err(|e| {
                    slug_error::slug_error!(
                        slug_error::ErrorTag::Input,
                        "Failed to read out-of-project bzlmod input file at {:?} as UTF-8: {}",
                        path,
                        e
                    )
                })?;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }

    /// Read the existence/metadata of an out-of-project (absolute-path) bzlmod input
    /// path through a cacheable DICE input. See `read_watched_abs_file_if_exists`.
    pub async fn read_watched_abs_path_metadata_if_exists(
        ctx: &mut DiceComputations<'_>,
        path: &Path,
    ) -> slug_error::Result<Arc<WatchedAbsPathMetadataValue>> {
        ctx.compute(&WatchedAbsPathMetadataKey {
            path: Arc::new(path.to_path_buf()),
        })
        .await?
    }

    /// Reads a project-relative file without going through a cell resolver.
    ///
    /// This is for bootstrap inputs that define the cell graph itself, such as
    /// root MODULE.bazel files. Normal build inputs should use `CellPath` reads.
    pub async fn read_project_file_if_exists(
        ctx: &mut DiceComputations<'_>,
        path: &ProjectRelativePath,
    ) -> slug_error::Result<Option<String>> {
        (ctx.compute(&ProjectReadFileKey(Arc::new(path.to_owned())))
            .await??
            .0)()
        .await
    }

    /// Reads project-relative file bytes without going through a cell resolver.
    ///
    /// This is for bootstrap/materialization inputs that are project-root
    /// relative and may not be valid UTF-8.
    pub async fn read_project_file_bytes_if_exists(
        ctx: &mut DiceComputations<'_>,
        path: &ProjectRelativePath,
    ) -> slug_error::Result<Option<Arc<Vec<u8>>>> {
        ctx.compute(&ProjectReadFileBytesKey(Arc::new(path.to_owned())))
            .await?
    }

    /// Reads project-relative path metadata without going through a cell resolver.
    ///
    /// This is for bootstrap inputs that define the cell graph itself, such as
    /// root MODULE.bazel files and bzlmod lockfile recorded inputs.
    pub async fn read_project_path_metadata_if_exists(
        ctx: &mut DiceComputations<'_>,
        path: &ProjectRelativePath,
    ) -> slug_error::Result<Option<RawPathMetadata<Arc<ProjectRelativePathBuf>>>> {
        ctx.compute(&ProjectPathMetadataKey(Arc::new(path.to_owned())))
            .await?
    }

    /// Reads sorted project-relative directory entry names without going through
    /// a cell resolver. This intentionally does not apply Buck ignore logic.
    pub async fn read_project_dir_entry_names(
        ctx: &mut DiceComputations<'_>,
        path: &ProjectRelativePath,
    ) -> slug_error::Result<Arc<Vec<String>>> {
        Ok(Arc::new(
            Self::read_project_dir_entries(ctx, path)
                .await?
                .iter()
                .map(|(name, _)| name.clone())
                .collect(),
        ))
    }

    /// Reads sorted project-relative directory entries without going through a
    /// cell resolver. This intentionally does not apply Buck ignore logic.
    pub async fn read_project_dir_entries(
        ctx: &mut DiceComputations<'_>,
        path: &ProjectRelativePath,
    ) -> slug_error::Result<Arc<Vec<(String, FileType)>>> {
        ctx.compute(&ProjectReadDirEntriesKey(Arc::new(path.to_owned())))
            .await?
    }

    /// Reads a project-relative file without going through a cell resolver.
    pub async fn read_project_file(
        ctx: &mut DiceComputations<'_>,
        path: &ProjectRelativePath,
    ) -> Result<String, FileReadError> {
        match Self::read_project_file_if_exists(ctx, path).await {
            Ok(result) => result.ok_or_else(|| FileReadError::NotFound(path.to_string())),
            Err(e) => Err(FileReadError::Buck(e)),
        }
    }

    /// Does not check if the path is ignored
    pub async fn read_path_metadata_if_exists(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> slug_error::Result<Option<RawPathMetadata>> {
        ctx.compute(&PathMetadataKey(path.to_owned())).await?
    }

    /// Does not check if the path is ignored
    pub async fn read_path_metadata(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> Result<RawPathMetadata, FileReadError> {
        match Self::read_path_metadata_if_exists(ctx, path).await {
            Ok(result) => result.ok_or_else(|| FileReadError::NotFound(path.to_string())),
            Err(e) => Err(FileReadError::Buck(e)),
        }
    }

    pub async fn is_ignored(
        ctx: &mut DiceComputations<'_>,
        path: CellPathRef<'_>,
    ) -> slug_error::Result<FileIgnoreResult> {
        get_delegated_file_ops(ctx, path.cell(), CheckIgnores::Yes)
            .await?
            .is_ignored(path.path())
            .await
    }

    pub async fn buildfiles(
        ctx: &mut DiceComputations<'_>,
        cell: CellName,
    ) -> slug_error::Result<Arc<[FileNameBuf]>> {
        ctx.get_buildfiles(cell).await
    }
}

#[derive(Debug, Display, Clone, Dupe, Copy, PartialEq, Eq, Hash, Allocative)]
pub(crate) enum CheckIgnores {
    Yes,
    No,
}

static READ_DIR_ACTIVE: AtomicUsize = AtomicUsize::new(0);
static READ_DIR_COMPLETED: AtomicUsize = AtomicUsize::new(0);
static READ_DIR_MAX_ACTIVE: AtomicUsize = AtomicUsize::new(0);

fn record_max_active(max: &AtomicUsize, active: usize) {
    let mut current = max.load(Ordering::Relaxed);
    while active > current {
        match max.compare_exchange_weak(current, active, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

fn read_dir_entry_stats(entries: &[SimpleDirEntry]) -> (usize, usize, usize, usize) {
    let mut files = 0;
    let mut dirs = 0;
    let mut symlinks = 0;
    let mut name_bytes = 0;
    for entry in entries {
        name_bytes += entry.file_name.as_str().len();
        if entry.file_type.is_dir() {
            dirs += 1;
        } else if entry.file_type.is_symlink() {
            symlinks += 1;
        } else {
            files += 1;
        }
    }
    (files, dirs, symlinks, name_bytes)
}

#[derive(Allocative)]
pub struct FileChangeTracker {
    files_to_dirty: HashSet<ReadFileKey>,
    project_files_to_dirty: HashSet<ProjectReadFileKey>,
    project_file_bytes_to_dirty: HashSet<ProjectReadFileBytesKey>,
    project_paths_to_dirty: HashSet<ProjectPathMetadataKey>,
    project_files_requiring_pre_config_commit: bool,
    dirs_to_dirty: HashSet<ReadDirKey>,
    paths_to_dirty: HashSet<PathMetadataKey>,
    exists_matching_exact_case_to_dirty: HashSet<ExistsMatchingExactCaseKey>,

    // Out-of-project (absolute-path) bzlmod inputs, dirtied by the per-sync
    // re-stat-diff over the daemon-owned registered-input registry.
    abs_files_to_dirty: HashSet<WatchedAbsFileKey>,
    abs_paths_to_dirty: HashSet<WatchedAbsPathMetadataKey>,
    abs_dir_entries_to_dirty: HashSet<WatchedAbsDirEntriesKey>,

    maybe_modified_dirs: HashSet<CellPath>,
}

impl FileChangeTracker {
    pub fn new() -> Self {
        Self {
            files_to_dirty: Default::default(),
            project_files_to_dirty: Default::default(),
            project_file_bytes_to_dirty: Default::default(),
            project_paths_to_dirty: Default::default(),
            project_files_requiring_pre_config_commit: false,
            dirs_to_dirty: Default::default(),
            paths_to_dirty: Default::default(),
            maybe_modified_dirs: Default::default(),
            exists_matching_exact_case_to_dirty: Default::default(),
            abs_files_to_dirty: Default::default(),
            abs_paths_to_dirty: Default::default(),
            abs_dir_entries_to_dirty: Default::default(),
        }
    }

    pub fn write_to_dice(mut self, ctx: &mut DiceTransactionUpdater) -> slug_error::Result<()> {
        // See comment on `dir_entries_changed_for_watchman_bug`
        for p in self.paths_to_dirty.clone() {
            if let Some(dir) = p.0.parent() {
                if self.maybe_modified_dirs.contains(&dir.to_owned()) {
                    self.entry_added_or_removed(p.0.clone());
                }
            }
        }

        ctx.changed(self.files_to_dirty)?;
        ctx.changed(self.project_files_to_dirty)?;
        ctx.changed(self.project_file_bytes_to_dirty)?;
        ctx.changed(self.project_paths_to_dirty)?;
        ctx.changed(self.dirs_to_dirty)?;
        ctx.changed(self.paths_to_dirty)?;
        ctx.changed(self.exists_matching_exact_case_to_dirty)?;
        ctx.changed(self.abs_files_to_dirty)?;
        ctx.changed(self.abs_paths_to_dirty)?;
        ctx.changed(self.abs_dir_entries_to_dirty)?;

        Ok(())
    }

    pub fn requires_pre_config_commit(&self) -> bool {
        self.project_files_requiring_pre_config_commit
    }

    fn entry_added_or_removed(&mut self, path: CellPath) {
        self.paths_to_dirty.insert(PathMetadataKey(path.clone()));
        self.exists_matching_exact_case_to_dirty
            .insert(ExistsMatchingExactCaseKey(path.clone()));
        let parent = path.parent();
        if let Some(parent) = parent {
            // The above can be None (validly!) if we have a cell we either create or delete.
            // That never happens in established repos, but if you are setting one up, it's not uncommon.
            // Since we don't include paths in different cells, the fact we don't dirty the parent
            // (which is in an enclosing cell) doesn't matter.
            self.insert_dir_keys(parent.to_owned());
        }
    }

    fn insert_dir_keys(&mut self, path: CellPath) {
        self.dirs_to_dirty.insert(ReadDirKey {
            path: path.clone(),
            check_ignores: CheckIgnores::No,
        });
        self.dirs_to_dirty.insert(ReadDirKey {
            path,
            check_ignores: CheckIgnores::Yes,
        });
    }

    pub fn file_added_or_removed(&mut self, path: CellPath) {
        self.file_contents_changed(path.clone());
        self.entry_added_or_removed(path);
    }

    pub fn dir_added_or_removed(&mut self, path: CellPath) {
        self.entry_added_or_removed(path);
    }

    pub fn file_contents_changed(&mut self, path: CellPath) {
        self.files_to_dirty
            .insert(ReadFileKey(Arc::new(path.clone())));
        self.paths_to_dirty.insert(PathMetadataKey(path.clone()));
    }

    pub fn project_file_added_or_removed(&mut self, path: ProjectRelativePathBuf) {
        self.project_file_contents_changed(path);
    }

    pub fn project_file_contents_changed(&mut self, path: ProjectRelativePathBuf) {
        if !is_bzlmod_config_project_file(&path) {
            return;
        }
        self.project_files_requiring_pre_config_commit = true;
        self.project_files_to_dirty
            .insert(ProjectReadFileKey(Arc::new(path.clone())));
        self.project_file_bytes_to_dirty
            .insert(ProjectReadFileBytesKey(Arc::new(path.clone())));
        self.project_paths_to_dirty
            .insert(ProjectPathMetadataKey(Arc::new(path)));
    }

    /// Normally, buck does not need the file watcher to tell it that a directory's entries have
    /// changed. However, in some cases file watcher want to force-invalidate directory listings,
    /// and so this exists. It should not normally be used.
    pub fn dir_entries_changed_force_invalidate(&mut self, path: CellPath) {
        self.insert_dir_keys(path);
    }

    /// Normally, we ignore directory modification events from file watchers and instead compute
    /// them ourselves when a file in the directory is reported as having been added or removed.
    /// However, watchman has a bug in which it sometimes incorrectly doesn't report files as having
    /// been added/removed. We work around this by implementing some logic that marks a directory
    /// listing as being invalid if both the directory and at least one of its entries is reported
    /// as having been modified.
    ///
    /// We cannot unconditionally respect directory modification events from the file watcher, as it
    /// is not aware of our ignore rules.
    pub fn dir_entries_changed_for_watchman_bug(&mut self, path: CellPath) {
        self.maybe_modified_dirs.insert(path);
    }

    /// Invalidate the cached content of an out-of-project (absolute-path) bzlmod
    /// input file. Called by the per-sync re-stat-diff when the on-disk content of a
    /// registered out-of-project input changed.
    pub fn abs_file_contents_changed(&mut self, path: PathBuf) {
        self.abs_files_to_dirty.insert(WatchedAbsFileKey {
            path: Arc::new(path.clone()),
        });
        if let Some(parent) = path.parent() {
            self.abs_dir_entries_to_dirty.insert(WatchedAbsDirEntriesKey {
                path: Arc::new(parent.to_path_buf()),
            });
        }
    }

    /// Invalidate the cached existence/metadata of an out-of-project (absolute-path)
    /// bzlmod input path (creation or deletion).
    pub fn abs_path_added_or_removed(&mut self, path: PathBuf) {
        self.abs_paths_to_dirty.insert(WatchedAbsPathMetadataKey {
            path: Arc::new(path.clone()),
        });
        self.abs_files_to_dirty.insert(WatchedAbsFileKey {
            path: Arc::new(path.clone()),
        });
        if let Some(parent) = path.parent() {
            self.abs_dir_entries_to_dirty.insert(WatchedAbsDirEntriesKey {
                path: Arc::new(parent.to_path_buf()),
            });
        }
    }
}

fn is_bzlmod_config_project_file(path: &ProjectRelativePath) -> bool {
    let path = path.as_str();
    if path == "MODULE.bazel"
        || path == "MODULE.bazel.lock"
        || path.ends_with("/MODULE.bazel")
        || path.ends_with("/MODULE.bazel.lock")
        || path.ends_with(".MODULE.bazel")
        || (path.contains("slug/registry/")
            && (path.ends_with("/source.json") || path.ends_with("/bazel_registry.json")))
    {
        return true;
    }

    BZLMOD_CONFIG_PROJECT_FILE_INPUTS
        .read()
        .is_ok_and(|paths| paths.iter().any(|registered| registered.as_str() == path))
}

/// The return value of a `ReadFileKey` computation.
///
/// Instead of the actual file contents, this is a closure that reads the actual file contents from
/// disk when invoked. This is done to ensure that we don't store the file contents in memory.
// FIXME(JakobDegen): `ReadFileKey` is not marked as transient if this returns an error, which is
// unfortunate.
#[derive(Clone, Dupe, Allocative)]
pub struct ReadFileProxy(
    #[allocative(skip)]
    Arc<dyn Fn() -> BoxFuture<'static, slug_error::Result<Option<String>>> + Send + Sync>,
);

impl ReadFileProxy {
    /// This is a convenience method that avoids a little bit of boilerplate around boxing, and
    /// cloning the captures
    pub fn new_with_captures<D, F>(data: D, c: impl Fn(D) -> F + Send + Sync + 'static) -> Self
    where
        D: Clone + Send + Sync + 'static,
        F: Future<Output = slug_error::Result<Option<String>>> + Send + 'static,
    {
        use futures::FutureExt;

        Self(Arc::new(move || {
            let data = data.clone();
            c(data).boxed()
        }))
    }
}

#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative)]
struct ReadFileKey(Arc<CellPath>);

#[async_trait]
impl Key for ReadFileKey {
    type Value = slug_error::Result<ReadFileProxy>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        get_delegated_file_ops(ctx, self.0.cell(), CheckIgnores::No)
            .await?
            .read_file_if_exists(ctx, self.0.path())
            .await
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        false
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("ProjectReadFileKey({})", _0)]
struct ProjectReadFileKey(Arc<ProjectRelativePathBuf>);

#[async_trait]
impl Key for ProjectReadFileKey {
    type Value = slug_error::Result<ReadFileProxy>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Ok(ReadFileProxy::new_with_captures(
            (
                self.0.as_ref().to_owned(),
                ctx.global_data().get_io_provider(),
            ),
            |(project_path, io)| async move { io.read_file_if_exists(project_path).await },
        ))
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        false
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("ProjectReadFileBytesKey({})", _0)]
struct ProjectReadFileBytesKey(Arc<ProjectRelativePathBuf>);

#[async_trait]
impl Key for ProjectReadFileBytesKey {
    type Value = slug_error::Result<Option<Arc<Vec<u8>>>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.global_data()
            .get_io_provider()
            .read_file_bytes_if_exists(self.0.as_ref().to_owned())
            .await
            .map(|content| content.map(Arc::new))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("ProjectPathMetadataKey({})", _0)]
struct ProjectPathMetadataKey(Arc<ProjectRelativePathBuf>);

#[async_trait]
impl Key for ProjectPathMetadataKey {
    type Value = slug_error::Result<Option<RawPathMetadata<Arc<ProjectRelativePathBuf>>>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.global_data()
            .get_io_provider()
            .read_path_metadata_if_exists(self.0.as_ref().to_owned())
            .await
            .map(|metadata| metadata.map(|metadata| metadata.map(Arc::new)))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

/// Content + digest of an out-of-project (absolute-path) bzlmod input file.
///
/// Unlike `ProjectReadFileKey`, these paths live outside the project root, so the
/// project file watcher does not proactively watch them. They are made cacheable
/// DICE inputs here; invalidation is injected by the per-sync re-stat-diff over the
/// daemon-owned registry of registered out-of-project bzlmod inputs (Plan 61
/// sub-plan 02 Phase A), mirroring Bazel's `ExternalDirtinessChecker`.
#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct WatchedAbsFileValue {
    pub content: Option<Arc<Vec<u8>>>,
    pub digest: Option<String>,
}

/// Existence of an out-of-project (absolute-path) bzlmod input path.
#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct WatchedAbsPathMetadataValue {
    pub exists: bool,
}

fn read_watched_abs_file_value(path: &Path) -> slug_error::Result<WatchedAbsFileValue> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WatchedAbsFileValue {
                content: None,
                digest: None,
            });
        }
        Err(e) => return Err(e.into()),
    };
    let digest = hex::encode(Sha256::digest(&bytes));
    Ok(WatchedAbsFileValue {
        content: Some(Arc::new(bytes)),
        digest: Some(digest),
    })
}

fn read_watched_abs_path_metadata_value(
    path: &Path,
) -> slug_error::Result<WatchedAbsPathMetadataValue> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(WatchedAbsPathMetadataValue { exists: true }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok(WatchedAbsPathMetadataValue { exists: false })
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("WatchedAbsFileKey({})", path.display())]
struct WatchedAbsFileKey {
    path: Arc<PathBuf>,
}

#[async_trait]
impl Key for WatchedAbsFileKey {
    type Value = slug_error::Result<Arc<WatchedAbsFileValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = read_watched_abs_file_value(&self.path)?;
        // Register so the per-command re-stat-diff observes edits to this
        // out-of-project input. No-op when no registry is installed (bootstrap/tests).
        if let Some(registry) = ctx.global_data().get_watched_abs_input_registry() {
            registry.register_file((*self.path).clone(), value.digest.clone());
        }
        Ok(Arc::new(value))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x.content == y.content && x.digest == y.digest,
            _ => false,
        }
    }

    // No `validity` override: this is a cacheable DICE input. The per-sync
    // re-stat-diff (Phase A.2) calls `FileChangeTracker::abs_file_contents_changed`
    // to invalidate it only when the on-disk content actually changes.
    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("WatchedAbsPathMetadataKey({})", path.display())]
struct WatchedAbsPathMetadataKey {
    path: Arc<PathBuf>,
}

#[async_trait]
impl Key for WatchedAbsPathMetadataKey {
    type Value = slug_error::Result<Arc<WatchedAbsPathMetadataValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = read_watched_abs_path_metadata_value(&self.path)?;
        if let Some(registry) = ctx.global_data().get_watched_abs_input_registry() {
            registry.register_path((*self.path).clone(), value.exists);
        }
        Ok(Arc::new(value))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

/// Sorted directory entries of an out-of-project (absolute-path) directory.
#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct WatchedAbsDirEntriesValue {
    pub entries: Arc<Vec<WatchedAbsDirEntry>>,
}

/// A single directory entry from a watched-absolute-path directory listing.
#[derive(Clone, Debug, PartialEq, Eq, Allocative)]
pub struct WatchedAbsDirEntry {
    pub file_name: String,
    pub file_type: FileType,
}

fn read_watched_abs_dir_entries(path: &Path) -> slug_error::Result<WatchedAbsDirEntriesValue> {
    let mut entries = Vec::new();
    let rd = match std::fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(WatchedAbsDirEntriesValue {
                entries: Arc::new(Vec::new()),
            });
        }
        Err(e) => {
            return Err(slug_error::slug_error!(
                slug_error::ErrorTag::Environment,
                "Failed to read directory {:?}: {}",
                path,
                e
            ));
        }
    };
    for entry in rd {
        let entry = entry.map_err(|e| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Environment,
                "Failed to read directory entry: {}",
                e
            )
        })?;
        let file_name = entry.file_name().into_string().map_err(|_| {
            slug_error::slug_error!(
                slug_error::ErrorTag::Environment,
                "Non-UTF8 filename in {:?}",
                path
            )
        })?;
        let entry_path = entry.path();
        let resolved = std::fs::metadata(&entry_path);
        let file_type = match resolved {
            Ok(md) if md.is_dir() => FileType::Directory,
            Ok(_) => FileType::File,
            Err(_) => {
                let st = entry.file_type().map_err(|e| {
                    slug_error::slug_error!(
                        slug_error::ErrorTag::Environment,
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
        entries.push(WatchedAbsDirEntry {
            file_name,
            file_type,
        });
    }
    entries.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(WatchedAbsDirEntriesValue {
        entries: Arc::new(entries),
    })
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("WatchedAbsDirEntriesKey({})", path.display())]
struct WatchedAbsDirEntriesKey {
    path: Arc<PathBuf>,
}

#[async_trait]
impl Key for WatchedAbsDirEntriesKey {
    type Value = slug_error::Result<Arc<WatchedAbsDirEntriesValue>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let value = read_watched_abs_dir_entries(&self.path)?;
        if let Some(registry) = ctx.global_data().get_watched_abs_input_registry() {
            registry.register_path((*self.path).clone(), true);
            for entry in value.entries.iter() {
                let entry_path = self.path.join(&entry.file_name);
                registry.register_path(entry_path, true);
            }
        }
        Ok(Arc::new(value))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

/// Public helper: compute a `WatchedAbsFileKey` for an absolute path and return
/// the raw bytes + SHA-256 digest. Used by external-cell delegates that need
/// DICE-tracked file reads outside the project root.
pub async fn compute_watched_abs_file(
    ctx: &mut DiceComputations<'_>,
    path: PathBuf,
) -> slug_error::Result<Arc<WatchedAbsFileValue>> {
    ctx.compute(&WatchedAbsFileKey {
        path: Arc::new(path),
    })
    .await?
}

/// Public helper: compute a `WatchedAbsPathMetadataKey` for an absolute path.
pub async fn compute_watched_abs_path_metadata(
    ctx: &mut DiceComputations<'_>,
    path: PathBuf,
) -> slug_error::Result<Arc<WatchedAbsPathMetadataValue>> {
    ctx.compute(&WatchedAbsPathMetadataKey {
        path: Arc::new(path),
    })
    .await?
}

/// Public helper: compute a `WatchedAbsDirEntriesKey` for an absolute path.
pub async fn compute_watched_abs_dir_entries(
    ctx: &mut DiceComputations<'_>,
    path: PathBuf,
) -> slug_error::Result<Arc<WatchedAbsDirEntriesValue>> {
    ctx.compute(&WatchedAbsDirEntriesKey {
        path: Arc::new(path),
    })
    .await?
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("ProjectReadDirEntriesKey({})", _0)]
struct ProjectReadDirEntriesKey(Arc<ProjectRelativePathBuf>);

impl Dupe for ProjectReadDirEntriesKey {
    fn dupe(&self) -> Self {
        Self(self.0.dupe())
    }
}

#[async_trait]
impl Key for ProjectReadDirEntriesKey {
    type Value = slug_error::Result<Arc<Vec<(String, FileType)>>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let mut entries: Vec<(String, FileType)> = ctx
            .global_data()
            .get_io_provider()
            .read_dir(self.0.as_ref().to_owned())
            .await?
            .into_iter()
            .map(|entry| (entry.file_name.to_string(), entry.file_type))
            .collect();
        entries.sort();
        Ok(Arc::new(entries))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(_x: &Self::Value) -> bool {
        false
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
#[display("{}", path)]
struct ReadDirKey {
    path: CellPath,
    check_ignores: CheckIgnores,
}

#[async_trait]
impl Key for ReadDirKey {
    type Value = slug_error::Result<ReadDirOutput>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let memory_checkpoints = slug_util::memory_checkpoint::enabled();
        let active = READ_DIR_ACTIVE.fetch_add(1, Ordering::Relaxed) + 1;
        record_max_active(&READ_DIR_MAX_ACTIVE, active);
        let result = match get_delegated_file_ops(ctx, self.path.cell(), self.check_ignores).await {
            Ok(file_ops) => file_ops
                .read_dir(ctx, self.path.as_ref().path())
                .await
                .map_err(slug_error::Error::from),
            Err(e) => Err(e),
        };
        let active = READ_DIR_ACTIVE.fetch_sub(1, Ordering::Relaxed) - 1;
        let completed = READ_DIR_COMPLETED.fetch_add(1, Ordering::Relaxed) + 1;

        if memory_checkpoints {
            let entries = result.as_ref().ok().map_or(0, |v| v.included.len());
            if entries >= 512 || completed.is_multiple_of(1000) {
                let (files, dirs, symlinks, name_bytes, ok) = match &result {
                    Ok(output) => {
                        let (files, dirs, symlinks, name_bytes) =
                            read_dir_entry_stats(&output.included);
                        (files, dirs, symlinks, name_bytes, 1)
                    }
                    Err(_) => (0, 0, 0, 0, 0),
                };
                slug_util::memory_checkpoint::checkpoint(
                    "read_dir_key",
                    [
                        ("active", active),
                        ("completed", completed),
                        ("max_active", READ_DIR_MAX_ACTIVE.load(Ordering::Relaxed)),
                        ("ok", ok),
                        ("entries", entries),
                        ("files", files),
                        ("dirs", dirs),
                        ("symlinks", symlinks),
                        ("name_bytes", name_bytes),
                        ("path_len", self.path.path().as_str().len()),
                        ("check_ignores", self.check_ignores as usize),
                    ],
                );
            }
        }

        result
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

#[derive(Clone, Display, Allocative, Debug, Eq, Hash, PartialEq)]
#[display("{}", _0)]
struct ExistsMatchingExactCaseKey(CellPath);

#[async_trait]
impl Key for ExistsMatchingExactCaseKey {
    type Value = slug_error::Result<bool>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        get_delegated_file_ops(ctx, self.0.cell(), CheckIgnores::Yes)
            .await?
            .exists_matching_exact_case(self.0.path(), ctx)
            .await
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }
}

#[derive(Clone, Display, Debug, Eq, Hash, PartialEq, Allocative)]
struct PathMetadataKey(CellPath);

#[async_trait]
impl Key for PathMetadataKey {
    type Value = slug_error::Result<Option<RawPathMetadata>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let res = get_delegated_file_ops(ctx, self.0.cell(), CheckIgnores::No)
            .await?
            .read_path_metadata_if_exists(ctx, self.0.as_ref().path())
            .await?;

        match res {
            Some(RawPathMetadata::Symlink {
                at: ref path,
                to: _,
            }) => {
                ctx.compute(&ReadFileKey(path.dupe())).await??;
            }
            _ => (),
        };

        Ok(res)
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        match (x, y) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }

    fn invalidation_source_priority() -> InvalidationSourcePriority {
        InvalidationSourcePriority::High
    }
}

/// out-of-line impl for DiceComputations::read_dir_ext so it doesn't add noise to the api
async fn read_dir_ext(
    ctx: &mut DiceComputations<'_>,
    path: CellPathRef<'_>,
) -> Result<ReadDirOutput, ReadDirError> {
    match DiceFileComputations::read_dir(ctx, path).await {
        Ok(v) => Ok(v),
        Err(e) => match extended_ignore_error(ctx, path).await {
            Some(e) => Err(e),
            None => Err(e.into()),
        },
    }
}

#[cfg(test)]
mod watched_abs_tests {
    use dice::UserComputationData;
    use dice::testing::DiceBuilder;

    use super::*;

    #[tokio::test]
    async fn watched_abs_file_is_cacheable_and_dirtied_by_tracker() -> slug_error::Result<()> {
        let dir = tempfile::Builder::new()
            .prefix("slug-plan61-watched-abs-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let path = dir.path().join("MODULE.bazel");
        std::fs::write(&path, "a").unwrap();

        let mut dice = DiceBuilder::new()
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let first = DiceFileComputations::read_watched_abs_file_if_exists(&mut dice, &path).await?;
        assert_eq!(first.as_deref(), Some("a"));

        // Cacheable: editing the file WITHOUT dirtying the key must not be re-read.
        std::fs::write(&path, "b").unwrap();
        let mut dice = dice.into_updater().commit().await;
        let cached =
            DiceFileComputations::read_watched_abs_file_if_exists(&mut dice, &path).await?;
        assert_eq!(
            cached.as_deref(),
            Some("a"),
            "value must stay cached until explicitly invalidated"
        );

        // Dirtying via the FileChangeTracker abs channel re-reads the new content.
        let mut updater = dice.into_updater();
        let mut tracker = FileChangeTracker::new();
        tracker.abs_file_contents_changed(path.clone());
        tracker.write_to_dice(&mut updater)?;
        let mut dice = updater.commit().await;
        let after = DiceFileComputations::read_watched_abs_file_if_exists(&mut dice, &path).await?;
        assert_eq!(after.as_deref(), Some("b"));
        Ok(())
    }

    #[tokio::test]
    async fn watched_abs_path_metadata_tracks_existence() -> slug_error::Result<()> {
        let dir = tempfile::Builder::new()
            .prefix("slug-plan61-watched-abs-meta-")
            .tempdir_in("/var/mnt/dev")
            .unwrap();
        let path = dir.path().join("MODULE.bazel");

        let mut dice = DiceBuilder::new()
            .build(UserComputationData::new())
            .unwrap()
            .commit()
            .await;

        let missing =
            DiceFileComputations::read_watched_abs_path_metadata_if_exists(&mut dice, &path)
                .await?;
        assert!(!missing.exists);

        std::fs::write(&path, "module(name = \"x\")\n").unwrap();
        let mut updater = dice.into_updater();
        let mut tracker = FileChangeTracker::new();
        tracker.abs_path_added_or_removed(path.clone());
        tracker.write_to_dice(&mut updater)?;
        let mut dice = updater.commit().await;
        let created =
            DiceFileComputations::read_watched_abs_path_metadata_if_exists(&mut dice, &path)
                .await?;
        assert!(created.exists);
        Ok(())
    }
}
