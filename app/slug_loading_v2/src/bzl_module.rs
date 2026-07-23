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
use dice::DiceComputations;
use dice::DiceTransaction;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use sha2::Digest;
use sha2::Sha256;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::syntax::AstModule;
use starlark::syntax::Dialect;

use crate::keys::BzlModuleEvalKey;
use crate::keys::BzlParseKey;
use crate::keys::LoadLabelResolutionKey;
use crate::keys::PackageLoadKey;
use crate::keys::WorkspaceDirectoryKey;
use crate::keys::WorkspaceDirectorySnapshotKey;
use crate::keys::WorkspaceDirectoryValue;
use crate::keys::WorkspaceFileKey;
use crate::keys::WorkspaceFileValue;
use crate::keys::WorkspaceSnapshotKey;
use crate::load_label::LoadLabel;
use crate::package::LoadedPackage;
use crate::package::PackageRecorder;
use crate::package::loading_globals;

/// Local-root loading operations over a caller-owned DICE transaction.
///
/// This intentionally owns no DICE instance or asynchronous runtime. The
/// workspace runtime supplies one committed transaction containing all file
/// observations for root and package loading.
#[derive(Clone)]
pub struct BzlModuleEvaluator {
    workspace: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBzlModule {
    pub path: PathBuf,
    pub loads: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
#[doc(hidden)]
pub struct LoadingError {
    message: String,
    absent: bool,
}

impl LoadingError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            absent: false,
        }
    }

    fn absent(path: &Path) -> Self {
        Self {
            message: format!("workspace file is absent: {}", path.display()),
            absent: true,
        }
    }

    fn is_absent(&self) -> bool {
        self.absent
    }
}

impl fmt::Display for LoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LoadingError {}

#[async_trait]
impl Key for WorkspaceFileKey {
    type Value = WorkspaceFileValue;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        match ctx
            .compute(&WorkspaceSnapshotKey {
                workspace: self.workspace.clone(),
            })
            .await
        {
            Ok(snapshot) => snapshot
                .files
                .get(&self.path)
                .cloned()
                .unwrap_or(WorkspaceFileValue::Absent),
            Err(error) => WorkspaceFileValue::ReadError(Arc::new(format!(
                "reading workspace snapshot for {}: {error}",
                self.path.display()
            ))),
        }
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
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
    source_digest: String,
}

#[derive(Allocative, Clone)]
#[doc(hidden)]
pub struct FrozenBzlModule {
    #[allocative(skip)]
    module: FrozenModule,
    path: PathBuf,
    loads: Vec<String>,
    fingerprint: String,
}

impl PartialEq for FrozenBzlModule {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }
}

impl Eq for FrozenBzlModule {}

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
                load,
            })
            .await
            .map_err(|error| anyhow::anyhow!("resolving local .bzl load through DICE: {error}"))?;
        let path = path
            .as_ref()
            .as_ref()
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
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(EvaluatedBzlModule {
            path: module.path.clone(),
            loads: module.loads.clone(),
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

    fn ensure_package(&self, package: &Path) -> anyhow::Result<()> {
        if !package.is_absolute() || !package.starts_with(&self.workspace) {
            anyhow::bail!(
                "package must be absolute and inside workspace {}: {}",
                self.workspace.display(),
                package.display()
            );
        }
        Ok(())
    }
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
            .map_err(|error| LoadingError::new(error.to_string()))?;
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
            let module = match ctx
                .compute(&BzlModuleEvalKey {
                    workspace: self.workspace.clone(),
                    path: resolved,
                })
                .await
            {
                Ok(value) => match value.as_ref() {
                    Ok(module) => module.clone(),
                    Err(error) => return Arc::new(Err(error.clone())),
                },
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            loaded_modules.push((load.as_str(), module));
        }

        Arc::new((|| {
            let ast = AstModule::parse(
                &self.path.display().to_string(),
                parsed.source.clone(),
                &Dialect::Standard,
            )
            .map_err(|error| LoadingError::new(error.to_string()))?;
            let module = Module::new();
            let fingerprint = transitive_fingerprint(
                &parsed.source_digest,
                loaded_modules.iter().map(|(load, module)| (*load, module)),
            );
            let loader = LocalBzlLoader {
                modules: loaded_modules
                    .iter()
                    .map(|(load, module)| (*load, module.module.dupe()))
                    .collect(),
            };
            {
                let mut evaluator = Evaluator::new(&module);
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
                fingerprint,
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
            let module = match ctx
                .compute(&BzlModuleEvalKey {
                    workspace: self.workspace.clone(),
                    path: resolved,
                })
                .await
            {
                Ok(value) => match value.as_ref() {
                    Ok(module) => module.clone(),
                    Err(error) => return Arc::new(Err(error.clone())),
                },
                Err(error) => return Arc::new(Err(LoadingError::new(error.to_string()))),
            };
            loaded_modules.push((load, module.module.dupe()));
        }

        Arc::new((|| {
            let ast = AstModule::parse(
                &build_file.display().to_string(),
                source.as_ref().clone(),
                &Dialect::Standard,
            )
            .map_err(|error| LoadingError::new(error.to_string()))?;
            let recorder = PackageRecorder::default();
            let module = Module::new();
            let loader = LocalBzlLoader {
                modules: loaded_modules
                    .iter()
                    .map(|(load, module)| (load.as_str(), module.dupe()))
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
            Ok(recorder.finish(
                self.package.clone(),
                build_file,
                loaded_modules
                    .iter()
                    .map(|(_, module)| module.dupe())
                    .collect(),
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

fn digest(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn transitive_fingerprint<'a>(
    source_digest: &str,
    loaded_modules: impl IntoIterator<Item = (&'a str, &'a FrozenBzlModule)>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source_digest.as_bytes());
    for (load, module) in loaded_modules {
        hasher.update(load.as_bytes());
        hasher.update(module.fingerprint.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}
