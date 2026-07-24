/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use compact_str::CompactString;
use dice::DiceComputations;
use dice::DiceTransaction;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::RootModuleGraphKey;
use slug_identity_v2::CanonicalLabel;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;
use starlark_map::small_set::SmallSet;

use crate::cycle_detector::BzlLoadCycle;
use crate::cycle_detector::BzlLoadCycleGuard;
use crate::glob::PackageListing;
use crate::keys::BzlModuleEvalKey;
use crate::keys::BzlParseKey;
use crate::keys::LoadLabelResolutionKey;
use crate::keys::PackageListingKey;
use crate::keys::PackageLoadKey;
use crate::keys::WorkspaceDirectoryEntryKind;
use crate::keys::WorkspaceDirectoryKey;
use crate::keys::WorkspaceDirectorySnapshotKey;
use crate::keys::WorkspaceDirectoryValue;
use crate::keys::WorkspaceFileKey;
use crate::keys::WorkspaceFileValue;
use crate::load_label::LoadLabel;
use crate::package::LoadedPackage;
use crate::package::PackageRecorder;
use crate::package::loading_globals;
use crate::provider::BzlEvaluationContext;

/// Local-root loading operations over a caller-owned DICE transaction.
///
/// This intentionally owns no DICE instance or asynchronous runtime. The
/// workspace runtime supplies one committed transaction containing all file
/// observations for root and package loading. Loading-capable transactions
/// must install [`crate::bzl_load_cycle_detector`] in their
/// `UserComputationData`; the modern DICE engine does not detect key cycles
/// itself.
#[derive(Clone)]
pub struct BzlModuleEvaluator {
    workspace: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBzlModule {
    pub path: PathBuf,
    pub loads: Vec<String>,
    pub manifest: BzlLoadManifest,
}

/// Stable identity for one local `.bzl` module in a workspace.
///
/// `label` is the canonical root-repository label and `workspace_path` is the
/// normalized absolute path DICE evaluated.  The value intentionally carries
/// no evaluator handle, so it can be used at semantic equality boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
pub struct BzlModuleIdentity {
    pub label: CanonicalLabel,
    pub workspace_path: PathBuf,
}

/// Flat, immutable loading provenance for one evaluated `.bzl` root.
///
/// Direct children retain source order with label-first deduplication.
/// `reachable` is a label-first, first-seen closure and is
/// deliberately flat: consumers do not need to walk an `Arc` DAG to compare
/// the loading semantics.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BzlLoadManifest {
    pub root: BzlModuleIdentity,
    pub direct_children: Arc<[BzlModuleIdentity]>,
    pub reachable: Arc<[BzlModuleIdentity]>,
    pub fingerprint: [u8; 32],
}

/// Parse-independent discovery result for a package's active BUILD basename.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct BuildFileCompanion {
    pub label: CanonicalLabel,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[doc(hidden)]
pub struct LoadingError {
    message: String,
    absent: bool,
    #[allocative(skip)]
    cycle: Option<BzlLoadCycle>,
}

impl LoadingError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            absent: false,
            cycle: None,
        }
    }

    fn absent(path: &Path) -> Self {
        Self {
            message: format!("workspace file is absent: {}", path.display()),
            absent: true,
            cycle: None,
        }
    }

    fn is_absent(&self) -> bool {
        self.absent
    }

    fn load_cycle(cycle: BzlLoadCycle) -> Self {
        Self {
            message: "cycle detected in extension files".to_owned(),
            absent: false,
            cycle: Some(cycle),
        }
    }

    fn with_package_cycle_origin(&self, workspace: &Path, package: &Path) -> Self {
        let Some(cycle) = &self.cycle else {
            return self.clone();
        };
        let package = package
            .strip_prefix(workspace)
            .unwrap_or(package)
            .to_string_lossy()
            .replace('\\', "/");
        let origin = if package.is_empty() {
            "BUILD".to_owned()
        } else {
            format!("{package}/BUILD")
        };
        let path = cycle
            .path
            .iter()
            .map(|key| {
                bzl_source_label(workspace, &key.path)
                    .unwrap_or_else(|_| key.path.display().to_string())
            })
            .collect::<Vec<_>>();
        let keys = cycle
            .keys
            .iter()
            .map(|key| {
                bzl_source_label(workspace, &key.path)
                    .unwrap_or_else(|_| key.path.display().to_string())
            })
            .collect::<Vec<_>>();
        Self::new(render_bzl_cycle(&origin, &path, &keys))
    }
}

impl fmt::Display for LoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadingError {}

fn load_error(load_label: &str, error: &LoadingError) -> LoadingError {
    if error.is_absent() {
        LoadingError::new(format!("cannot load '{load_label}': no such file"))
    } else {
        error.clone()
    }
}

fn render_bzl_cycle(origin: &str, path: &[String], cycle: &[String]) -> String {
    // Bazel 9 source of truth:
    // BzlLoadCycleReporter + AbstractLabelCycleReporter::printCycle.
    let mut message = format!("cycle detected in extension files: \n    {origin}");
    for label in path {
        message.push_str("\n    ");
        message.push_str(label);
    }
    let Some((first, rest)) = cycle.split_first() else {
        return message;
    };
    message.push_str("\n.-> ");
    message.push_str(first);
    if rest.is_empty() {
        message.push_str(" [self-edge]\n`--");
        return message;
    }
    for label in rest {
        message.push_str("\n|   ");
        message.push_str(label);
    }
    message.push_str("\n`-- ");
    message.push_str(first);
    message
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct BzlLoadCyclePoisonKey;

impl fmt::Display for BzlLoadCyclePoisonKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bzl-load-cycle-poison")
    }
}

#[async_trait]
impl Key for BzlLoadCyclePoisonKey {
    type Value = ();

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
    }

    fn equality(_x: &Self::Value, _y: &Self::Value) -> bool {
        true
    }

    fn validity(_value: &Self::Value) -> bool {
        false
    }
}

#[async_trait]
impl Key for WorkspaceDirectoryKey {
    type Value = WorkspaceDirectoryValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match ctx
            .compute(&WorkspaceDirectorySnapshotKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(snapshot) => snapshot
                .directories
                .get(&self.directory)
                .cloned()
                .unwrap_or(WorkspaceDirectoryValue::Absent),
            Err(error) => WorkspaceDirectoryValue::ReadError(Arc::new(format!(
                "reading workspace directory snapshot for {}: {error}",
                self.directory.display()
            ))),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

#[async_trait]
impl Key for PackageListingKey {
    type Value = LoadResult<PackageListing>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        if !self.workspace.is_absolute()
            || !self.package.is_absolute()
            || !self.package.starts_with(&self.workspace)
            || self.package.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::CurDir | std::path::Component::ParentDir
                )
            })
        {
            return Arc::new(Err(LoadingError::new(format!(
                "package listing key requires a normalized absolute package inside workspace {}: {}",
                self.workspace.display(),
                self.package.display()
            ))));
        }

        let mut regular_files = Vec::new();
        let mut directories = Vec::new();
        let mut watched_directories = Vec::new();
        let mut subpackages = Vec::new();
        let mut pending = vec![(self.package.clone(), String::new())];

        while let Some((directory, relative_directory)) = pending.pop() {
            let value = match ctx
                .compute(&WorkspaceDirectoryKey {
                    workspace: self.workspace.clone(),
                    directory: directory.clone(),
                })
                .await
            {
                Ok(value) => value,
                Err(error) => {
                    return Arc::new(Err(LoadingError::new(format!(
                        "reading package directory {} through DICE: {error}",
                        directory.display()
                    ))));
                }
            };
            let entries = match value {
                WorkspaceDirectoryValue::Present(entries) => entries,
                WorkspaceDirectoryValue::Absent => {
                    return Arc::new(Err(LoadingError::new(format!(
                        "package directory is absent: {}",
                        directory.display()
                    ))));
                }
                WorkspaceDirectoryValue::ReadError(error) => {
                    return Arc::new(Err(LoadingError::new(format!(
                        "reading package directory {}: {error}",
                        directory.display()
                    ))));
                }
            };

            watched_directories.push(CompactString::new(&relative_directory));
            let is_subpackage = !relative_directory.is_empty()
                && entries.iter().any(|entry| {
                    matches!(entry.name.as_str(), "BUILD.bazel" | "BUILD")
                        && entry.kind == WorkspaceDirectoryEntryKind::RegularFile
                });
            if is_subpackage {
                subpackages.push(CompactString::new(&relative_directory));
                continue;
            }

            if !relative_directory.is_empty() {
                directories.push(CompactString::new(&relative_directory));
            }
            for entry in entries.iter().rev() {
                let relative = if relative_directory.is_empty() {
                    entry.name.to_string()
                } else {
                    format!("{relative_directory}/{}", entry.name)
                };
                match entry.kind {
                    WorkspaceDirectoryEntryKind::RegularFile => {
                        regular_files.push(CompactString::new(&relative));
                    }
                    WorkspaceDirectoryEntryKind::Directory => {
                        pending.push((directory.join(entry.name.as_str()), relative));
                    }
                    WorkspaceDirectoryEntryKind::Symlink => {
                        return Arc::new(Err(LoadingError::new(format!(
                            "symlink entries are unsupported while listing package {}: {relative}",
                            self.package.display()
                        ))));
                    }
                    WorkspaceDirectoryEntryKind::Other => {
                        return Arc::new(Err(LoadingError::new(format!(
                            "special filesystem entries are unsupported while listing package {}: {relative}",
                            self.package.display()
                        ))));
                    }
                }
            }
        }

        Arc::new(Ok(PackageListing::new(
            regular_files,
            directories,
            watched_directories,
            subpackages,
        )))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x.as_ref(), y.as_ref()), (Ok(x), Ok(y)) if x == y)
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

async fn observed_file(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    path: &Path,
) -> Result<Arc<String>, LoadingError> {
    let value = ctx
        .compute(&WorkspaceFileKey {
            workspace: workspace.to_path_buf(),
            path: path.to_path_buf(),
        })
        .await
        .map_err(|error| {
            LoadingError::new(format!("reading {} through DICE: {error}", path.display()))
        })?;
    match value {
        WorkspaceFileValue::Present(source) => Ok(source.clone()),
        WorkspaceFileValue::Absent => Err(LoadingError::absent(path)),
        WorkspaceFileValue::ReadError(error) => Err(LoadingError::new(format!(
            "reading {}: {error}",
            path.display()
        ))),
    }
}

#[derive(Allocative, Clone, PartialEq, Eq)]
#[doc(hidden)]
pub struct ParsedBzl {
    source: String,
    loads: Vec<String>,
    source_digest: [u8; 32],
}

#[derive(Debug, Allocative, Clone)]
#[doc(hidden)]
pub struct FrozenBzlModule {
    #[allocative(skip)]
    module: FrozenModule,
    path: PathBuf,
    loads: Vec<String>,
    manifest: BzlLoadManifest,
    /// Kept separately from `manifest`: frozen module pointers are lifetime
    /// ownership, never semantic equality.
    retained_bzl_modules: Arc<[FrozenBzlLifetimeEntry]>,
}

/// Lifetime-only pairing kept structurally aligned with its identity.
#[derive(Debug, Allocative, Clone)]
pub(crate) struct FrozenBzlLifetimeEntry {
    identity: BzlModuleIdentity,
    #[allocative(skip)]
    module: FrozenModule,
}

impl PartialEq for FrozenBzlModule {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest
    }
}

impl Eq for FrozenBzlModule {}

impl FrozenBzlModule {
    fn lifetime_modules(&self) -> impl Iterator<Item = (&BzlModuleIdentity, FrozenModule)> {
        std::iter::once((&self.manifest.root, self.module.dupe())).chain(
            self.retained_bzl_modules
                .iter()
                .map(|entry| (&entry.identity, entry.module.dupe())),
        )
    }
}

impl BzlLoadManifest {
    fn new(
        root: BzlModuleIdentity,
        source_digest: [u8; 32],
        direct_modules: impl IntoIterator<Item = impl std::borrow::Borrow<FrozenBzlModule>>,
    ) -> Self {
        let direct_modules = direct_modules.into_iter().collect::<Vec<_>>();
        let mut direct_indices = Vec::with_capacity(direct_modules.len());
        let mut direct_children = Vec::with_capacity(direct_modules.len());
        let mut direct_seen_labels = SmallSet::with_capacity(direct_modules.len());
        for (index, module) in direct_modules.iter().enumerate() {
            let identity = &module.borrow().manifest.root;
            if direct_seen_labels.insert(identity.label.clone()) {
                direct_children.push(identity.clone());
                direct_indices.push(index);
            }
        }
        let mut reachable = vec![root.clone()];
        let mut seen_labels = SmallSet::with_capacity(direct_children.len() + 1);
        seen_labels.insert(root.label.clone());
        for module in &direct_modules {
            for identity in module.borrow().manifest.reachable.iter() {
                if seen_labels.insert(identity.label.clone()) {
                    reachable.push(identity.clone());
                }
            }
        }
        let fingerprint = manifest_fingerprint(
            source_digest,
            direct_indices
                .iter()
                .map(|index| direct_modules[*index].borrow()),
        );
        Self {
            root,
            direct_children: direct_children.into(),
            reachable: reachable.into(),
            fingerprint,
        }
    }
}

type LoadResult<T> = Arc<Result<T, LoadingError>>;

impl BzlModuleEvaluator {
    pub fn new(workspace: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let workspace = workspace.into();
        let original_workspace = workspace.clone();
        let workspace = workspace.canonicalize().map_err(|error| {
            anyhow::anyhow!(
                "canonicalizing workspace for .bzl loading: {}: {error}",
                original_workspace.display()
            )
        })?;
        Ok(Self { workspace })
    }

    /// Evaluate a Bazel local load label from one package directory.
    ///
    pub async fn evaluate_load(
        &self,
        transaction: &mut DiceTransaction,
        requesting_package: impl AsRef<Path>,
        load: impl AsRef<str>,
    ) -> anyhow::Result<EvaluatedBzlModule> {
        let requesting_package = requesting_package.as_ref().to_path_buf();
        self.ensure_package(&requesting_package)?;
        let workspace = self.workspace.clone();
        let load = load.as_ref().to_owned();
        let path = transaction
            .compute(&LoadLabelResolutionKey {
                workspace: workspace.clone(),
                requesting_package,
                load: load.clone(),
            })
            .await
            .map_err(|error| anyhow::anyhow!("resolving local .bzl load through DICE: {error}"))?;
        let path = path
            .as_ref()
            .as_ref()
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let load_label = bzl_source_label(&workspace, path)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        let module = transaction
            .compute(&BzlModuleEvalKey {
                workspace,
                path: path.clone(),
            })
            .await
            .map_err(|error| anyhow::anyhow!("evaluating local .bzl load through DICE: {error}"))?;
        let module = module
            .as_ref()
            .as_ref()
            .map_err(|error| anyhow::anyhow!(load_error(&load_label, error).to_string()))?;
        Ok(EvaluatedBzlModule {
            path: module.path.clone(),
            loads: module.loads.clone(),
            manifest: module.manifest.clone(),
        })
    }

    /// Evaluate a package BUILD file with the currently supported Bazel globals
    /// and the same DICE-backed local `.bzl` graph used by [`Self::evaluate_load`].
    pub async fn evaluate_package(
        &self,
        transaction: &mut DiceTransaction,
        package: impl AsRef<Path>,
    ) -> anyhow::Result<LoadedPackage> {
        let package = package.as_ref().to_path_buf();
        self.ensure_package(&package)?;
        let workspace = self.workspace.clone();
        let package = transaction
            .compute(&PackageLoadKey { workspace, package })
            .await
            .map_err(|error| anyhow::anyhow!("loading BUILD package through DICE: {error}"))?;
        package
            .as_ref()
            .as_ref()
            .map(Clone::clone)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    /// Return the active BUILD companion for `package` using only its injected
    /// direct-directory observation. This is deliberately parse-independent:
    /// a broken companion BUILD file is still discoverable and no package load
    /// is computed as a side effect.
    pub async fn discover_build_file_companion(
        &self,
        transaction: &mut DiceTransaction,
        package: impl AsRef<Path>,
    ) -> anyhow::Result<Option<BuildFileCompanion>> {
        let package = package.as_ref().to_path_buf();
        self.ensure_package(&package)?;
        let value = transaction
            .compute(&WorkspaceDirectoryKey {
                workspace: self.workspace.clone(),
                directory: package.clone(),
            })
            .await
            .map_err(|error| anyhow::anyhow!("reading package directory through DICE: {error}"))?;
        companion_from_directory_value(&self.workspace, &package, &value)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    fn ensure_package(&self, package: &Path) -> anyhow::Result<()> {
        validate_normalized_workspace_package(&self.workspace, package)
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }
}

/// Discover a package's active BUILD companion using the existing
/// `WorkspaceDirectoryKey` only. Missing directories and missing BUILD files
/// are not errors for a load-label package, while observation read failures
/// remain errors.
pub async fn discover_build_file_companion(
    ctx: &mut DiceComputations<'_>,
    workspace: &Path,
    package: &Path,
) -> Result<Option<BuildFileCompanion>, LoadingError> {
    validate_normalized_workspace_package(workspace, package)?;
    let value = ctx
        .compute(&WorkspaceDirectoryKey {
            workspace: workspace.to_path_buf(),
            directory: package.to_path_buf(),
        })
        .await
        .map_err(|error| {
            LoadingError::new(format!("reading package directory through DICE: {error}"))
        })?;
    companion_from_directory_value(workspace, package, &value)
}

fn companion_from_directory_value(
    workspace: &Path,
    package: &Path,
    value: &WorkspaceDirectoryValue,
) -> Result<Option<BuildFileCompanion>, LoadingError> {
    let entries = match value {
        WorkspaceDirectoryValue::Present(entries) => entries,
        WorkspaceDirectoryValue::Absent => return Ok(None),
        WorkspaceDirectoryValue::ReadError(error) => {
            return Err(LoadingError::new(format!(
                "reading package directory {}: {error}",
                package.display()
            )));
        }
    };
    let Some(basename) = ["BUILD.bazel", "BUILD"].into_iter().find(|name| {
        entries.iter().any(|entry| {
            entry.name.as_str() == *name
                && matches!(
                    entry.kind,
                    WorkspaceDirectoryEntryKind::RegularFile | WorkspaceDirectoryEntryKind::Symlink
                )
        })
    }) else {
        return Ok(None);
    };
    let path = package.join(basename);
    Ok(Some(BuildFileCompanion {
        label: canonical_root_label(&build_source_label(workspace, &path)?)?,
        path,
    }))
}

#[async_trait]
impl Key for BzlParseKey {
    type Value = LoadResult<ParsedBzl>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let source = match observed_file(ctx, &self.workspace, &self.path).await {
            Ok(source) => source,
            Err(error) => return Arc::new(Err(error)),
        };
        Arc::new((|| {
            let ast = AstModule::parse(
                &self.path.display().to_string(),
                source.as_ref().clone(),
                &Dialect::Standard,
            )
            .map_err(|error| {
                let module = self
                    .path
                    .strip_prefix(&self.workspace)
                    .unwrap_or(&self.path)
                    .to_string_lossy()
                    .replace('\\', "/");
                LoadingError::new(format!("{error}\ncompilation of module '{module}' failed"))
            })?;
            Ok(ParsedBzl {
                source_digest: digest(&source),
                source: source.as_ref().clone(),
                loads: ast
                    .loads()
                    .into_iter()
                    .map(|load| load.module_id.to_owned())
                    .collect(),
            })
        })())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[async_trait]
impl Key for LoadLabelResolutionKey {
    type Value = LoadResult<PathBuf>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Arc::new(resolve_local_load(
            &self.workspace,
            &self.requesting_package,
            &self.load,
        ))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[async_trait]
impl Key for BzlModuleEvalKey {
    type Value = LoadResult<FrozenBzlModule>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let parsed = match ctx
            .compute(&BzlParseKey {
                workspace: self.workspace.clone(),
                path: self.path.clone(),
            })
            .await
        {
            Ok(value) => match value.as_ref() {
                Ok(parsed) => parsed.clone(),
                Err(error) => return Arc::new(Err(error.clone())),
            },
            Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
        };

        let mut loaded_modules = Vec::with_capacity(parsed.loads.len());
        for load in &parsed.loads {
            let resolved = match ctx
                .compute(&LoadLabelResolutionKey {
                    workspace: self.workspace.clone(),
                    requesting_package: self.path.parent().unwrap_or(&self.workspace).to_path_buf(),
                    load: load.clone(),
                })
                .await
            {
                Ok(value) => match value.as_ref() {
                    Ok(path) => path.clone(),
                    Err(error) => return Arc::new(Err(error.clone())),
                },
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            let load_label = match bzl_source_label(&self.workspace, &resolved) {
                Ok(label) => label,
                Err(error) => return Arc::new(Err(error)),
            };
            let child = BzlModuleEvalKey {
                workspace: self.workspace.clone(),
                path: resolved,
            };
            let cycle_guard = match ctx.cycle_guard::<BzlLoadCycleGuard>() {
                Ok(guard) => guard,
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            let result = match cycle_guard {
                Some(guard) => match guard.guard_this(ctx.compute(&child)).await {
                    Ok(result) => result,
                    Err(cycle) => {
                        let _unused = ctx.compute(&BzlLoadCyclePoisonKey).await;
                        return Arc::new(Err(LoadingError::load_cycle(cycle)));
                    }
                },
                None => ctx.compute(&child).await,
            };
            let module = match result {
                Ok(value) => match value.as_ref() {
                    Ok(module) => module.clone(),
                    Err(error) => return Arc::new(Err(load_error(&load_label, error))),
                },
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            loaded_modules.push((load.clone(), module));
        }

        Arc::new((|| {
            let ast = AstModule::parse(
                &self.path.display().to_string(),
                parsed.source.clone(),
                &Dialect::Standard,
            )
            .map_err(|error| LoadingError::new(error.to_string()))?;
            let module = Module::new();
            let manifest = BzlLoadManifest::new(
                bzl_module_identity(&self.workspace, &self.path)?,
                parsed.source_digest,
                loaded_modules.iter().map(|(_, module)| module),
            );
            let loader = LocalBzlLoader {
                modules: loaded_modules
                    .iter()
                    .map(|(load, module)| (load.as_str(), module.module.dupe()))
                    .collect(),
            };
            let evaluation_context = BzlEvaluationContext::new(
                bzl_source_label(&self.workspace, &self.path)
                    .map_err(|error| LoadingError::new(error.to_string()))?,
            );
            {
                let mut evaluator = Evaluator::new(&module);
                evaluator.extra = Some(&evaluation_context);
                evaluator.set_loader(&loader);
                evaluator
                    .eval_module(ast, &loading_globals())
                    .map_err(|error| LoadingError::new(error.to_string()))?;
            }
            let module = module
                .freeze()
                .map_err(|error| LoadingError::new(format!("{error:?}")))?;
            Ok(FrozenBzlModule {
                module,
                path: self.path.clone(),
                loads: parsed.loads.clone(),
                retained_bzl_modules: retained_module_closure(&loaded_modules),
                manifest,
            })
        })())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

#[async_trait]
impl Key for PackageLoadKey {
    type Value = LoadResult<LoadedPackage>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let root_module_graph_value = match ctx
            .compute(&RootModuleGraphKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(value) => value,
            Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
        };
        let root_module_graph = match root_module_graph_value.as_ref() {
            Ok(graph) => graph,
            Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
        };
        let _repository_mapping = &root_module_graph.repository_mapping;
        let listing = match ctx
            .compute(&PackageListingKey {
                workspace: self.workspace.clone(),
                package: self.package.clone(),
            })
            .await
        {
            Ok(value) => match value.as_ref() {
                Ok(listing) => listing.dupe(),
                Err(error) => return Arc::new(Err(error.clone())),
            },
            Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
        };
        let primary_build = self.package.join("BUILD.bazel");
        let fallback_build = self.package.join("BUILD");
        let (build_file, source) = match observed_file(ctx, &self.workspace, &primary_build).await {
            Ok(source) => (primary_build, source),
            Err(error) if error.is_absent() => {
                match observed_file(ctx, &self.workspace, &fallback_build).await {
                    Ok(source) => (fallback_build, source),
                    Err(error) if error.is_absent() => {
                        return Arc::new(Err(LoadingError::new(format!(
                            "no BUILD.bazel or BUILD file in package {}",
                            self.package.display()
                        ))));
                    }
                    Err(error) => return Arc::new(Err(error)),
                }
            }
            Err(error) => return Arc::new(Err(error)),
        };
        let ast = match AstModule::parse(
            &build_file.display().to_string(),
            source.as_ref().clone(),
            &Dialect::Standard,
        ) {
            Ok(ast) => ast,
            Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
        };

        let mut loaded_modules = Vec::new();
        for load in ast.loads() {
            let load = load.module_id.to_owned();
            let resolved = match ctx
                .compute(&LoadLabelResolutionKey {
                    workspace: self.workspace.clone(),
                    requesting_package: self.package.clone(),
                    load: load.clone(),
                })
                .await
            {
                Ok(value) => match value.as_ref() {
                    Ok(path) => path.clone(),
                    Err(error) => return Arc::new(Err(error.clone())),
                },
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            let load_label = match bzl_source_label(&self.workspace, &resolved) {
                Ok(label) => label,
                Err(error) => return Arc::new(Err(error)),
            };
            let module = match ctx
                .compute(&BzlModuleEvalKey {
                    workspace: self.workspace.clone(),
                    path: resolved,
                })
                .await
            {
                Ok(value) => match value.as_ref() {
                    Ok(module) => module.clone(),
                    Err(error) => {
                        return Arc::new(Err(load_error(&load_label, error)
                            .with_package_cycle_origin(&self.workspace, &self.package)));
                    }
                },
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            loaded_modules.push((load, module));
        }

        Arc::new((|| {
            let ast = AstModule::parse(
                &build_file.display().to_string(),
                source.as_ref().clone(),
                &Dialect::Standard,
            )
            .map_err(|error| LoadingError::new(error.to_string()))?;
            let package_label = self
                .package
                .strip_prefix(&self.workspace)
                .map_err(|_| {
                    LoadingError::new(format!(
                        "package is outside workspace: {}",
                        self.package.display()
                    ))
                })?
                .to_string_lossy()
                .replace('\\', "/");
            let recorder = PackageRecorder::new(listing, package_label);
            let module = Module::new();
            let loader = LocalBzlLoader {
                modules: loaded_modules
                    .iter()
                    .map(|(load, module)| (load.as_str(), module.module.dupe()))
                    .collect(),
            };
            {
                let mut evaluator = Evaluator::new(&module);
                evaluator.extra = Some(&recorder);
                evaluator.set_loader(&loader);
                evaluator
                    .eval_module(ast, &loading_globals())
                    .map_err(|error| LoadingError::new(error.to_string()))?;
            }
            let direct_load_roots = first_seen_direct_roots(&loaded_modules);
            let retained_bzl_modules = flattened_lifetime_closure(&loaded_modules);
            let reachable_loads = retained_bzl_modules
                .iter()
                .map(|entry| entry.identity.clone())
                .collect::<Vec<_>>();
            let load_fingerprint = package_load_fingerprint(&loaded_modules);
            Ok(recorder.finish(
                self.package.clone(),
                build_file,
                direct_load_roots.into(),
                reachable_loads.into(),
                load_fingerprint,
                retained_bzl_modules.into(),
            ))
        })())
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn validity(value: &Self::Value) -> bool {
        value.is_ok()
    }
}

struct LocalBzlLoader<'a> {
    modules: Vec<(&'a str, FrozenModule)>,
}

impl starlark::eval::FileLoader for LocalBzlLoader<'_> {
    fn load(&self, path: &str) -> starlark::Result<FrozenModule> {
        self.modules
            .iter()
            .find(|(load, _)| *load == path)
            .map(|(_, module)| module.dupe())
            .ok_or_else(|| {
                starlark::Error::new_other(anyhow::anyhow!(
                    "local .bzl loader does not know module `{path}`"
                ))
            })
    }
}

fn resolve_local_load(
    workspace: &Path,
    requesting_package: &Path,
    load: &str,
) -> Result<PathBuf, LoadingError> {
    let label = if let Some(relative) = load.strip_prefix(':') {
        let relative_label =
            LoadLabel::parse(&format!("//:{relative}")).map_err(LoadingError::new)?;
        let path = requesting_package.join(relative_label.label().target().as_str());
        return ensure_within_workspace(workspace, path, load);
    } else {
        LoadLabel::parse(load).map_err(LoadingError::new)?
    };
    if !label.label().repo().is_root() {
        return Err(LoadingError::new(format!(
            "external repository load is not available in the local Stage 4 loader: {load}"
        )));
    }
    let package = label.label().package().as_str();
    let path = workspace
        .join(package)
        .join(label.label().target().as_str());
    ensure_within_workspace(workspace, path, load)
}

fn bzl_source_label(workspace: &Path, path: &Path) -> Result<String, LoadingError> {
    let relative = path.strip_prefix(workspace).map_err(|_| {
        LoadingError::new(format!(
            ".bzl source is outside workspace {}: {}",
            workspace.display(),
            path.display()
        ))
    })?;
    let target = relative
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            LoadingError::new(format!("invalid .bzl source path: {}", path.display()))
        })?;
    let package = relative
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_string_lossy()
        .replace('\\', "/");
    Ok(format!("//{package}:{target}"))
}

fn build_source_label(workspace: &Path, path: &Path) -> Result<String, LoadingError> {
    bzl_source_label(workspace, path)
}

fn bzl_module_identity(workspace: &Path, path: &Path) -> Result<BzlModuleIdentity, LoadingError> {
    Ok(BzlModuleIdentity {
        label: canonical_root_label(&bzl_source_label(workspace, path)?)?,
        workspace_path: path.to_path_buf(),
    })
}

fn validate_normalized_workspace_package(
    workspace: &Path,
    package: &Path,
) -> Result<(), LoadingError> {
    if !workspace.is_absolute()
        || !package.is_absolute()
        || !package.starts_with(workspace)
        || workspace.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || has_cur_or_parent_component(workspace)
        || package.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || has_cur_or_parent_component(package)
    {
        return Err(LoadingError::new(format!(
            "package must be a normalized absolute path inside workspace {}: {}",
            workspace.display(),
            package.display()
        )));
    }
    Ok(())
}

fn has_cur_or_parent_component(path: &Path) -> bool {
    path.as_os_str()
        .as_encoded_bytes()
        .split(|byte| *byte == b'/')
        .any(|component| matches!(component, b"." | b".."))
}

fn canonical_root_label(label: &str) -> Result<CanonicalLabel, LoadingError> {
    CanonicalLabel::parse(&format!("@@{label}")).map_err(LoadingError::new)
}

fn ensure_within_workspace(
    workspace: &Path,
    path: PathBuf,
    load: &str,
) -> Result<PathBuf, LoadingError> {
    if !path.starts_with(workspace) {
        return Err(LoadingError::new(format!(
            "load label escapes workspace: {load}"
        )));
    }
    Ok(path)
}

fn digest(source: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher.finalize().into()
}

fn manifest_fingerprint(
    source_digest: [u8; 32],
    direct_modules: impl IntoIterator<Item = impl std::borrow::Borrow<FrozenBzlModule>>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"slug-v2-bzl-load-manifest\0");
    hasher.update(source_digest);
    for module in direct_modules {
        let module = module.borrow();
        fingerprint_identity(&mut hasher, &module.manifest.root);
        hasher.update(module.manifest.fingerprint);
    }
    hasher.finalize().into()
}

fn package_load_fingerprint<'a>(direct_loads: &'a [(String, FrozenBzlModule)]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"slug-v2-package-load-manifest\0");
    let mut seen_labels = SmallSet::with_capacity(direct_loads.len());
    for (_, module) in direct_loads {
        if seen_labels.insert(module.manifest.root.label.clone()) {
            fingerprint_identity(&mut hasher, &module.manifest.root);
            hasher.update(module.manifest.fingerprint);
        }
    }
    hasher.finalize().into()
}

fn fingerprint_identity(hasher: &mut Sha256, identity: &BzlModuleIdentity) {
    let label = identity.label.to_string();
    let label = label.as_bytes();
    let path = identity.workspace_path.as_os_str().as_encoded_bytes();
    hasher.update((label.len() as u64).to_be_bytes());
    hasher.update(label);
    hasher.update((path.len() as u64).to_be_bytes());
    hasher.update(path);
}

fn first_seen_direct_roots(loaded_modules: &[(String, FrozenBzlModule)]) -> Vec<BzlModuleIdentity> {
    let mut seen_labels = SmallSet::with_capacity(loaded_modules.len());
    loaded_modules
        .iter()
        .filter_map(|(_, module)| {
            seen_labels
                .insert(module.manifest.root.label.clone())
                .then(|| module.manifest.root.clone())
        })
        .collect()
}

fn retained_module_closure(
    loaded_modules: &[(String, FrozenBzlModule)],
) -> Arc<[FrozenBzlLifetimeEntry]> {
    flattened_lifetime_closure(loaded_modules).into()
}

fn flattened_lifetime_closure(
    loaded_modules: &[(String, FrozenBzlModule)],
) -> Vec<FrozenBzlLifetimeEntry> {
    let mut seen_labels = SmallSet::new();
    let mut entries = Vec::new();
    for (_, loaded_module) in loaded_modules {
        for (identity, module) in loaded_module.lifetime_modules() {
            if seen_labels.insert(identity.label.clone()) {
                entries.push(FrozenBzlLifetimeEntry {
                    identity: identity.clone(),
                    module,
                });
            }
        }
    }
    entries
}
