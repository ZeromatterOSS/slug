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
use anyhow::Context;
use async_trait::async_trait;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceComputations;
use dice::Key;
use dice_futures::cancellation::CancellationContext;
use slug_analysis_v2::AnalysisResult;
use slug_analysis_v2::ConfigurationKey;
use slug_analysis_v2::ConfiguredTargetKey;
use slug_analysis_v2::analyze_loaded_rule;
use slug_identity_v2::CanonicalLabel;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::BzlModuleEvaluator;
use slug_loading_v2::LoadedPackage;

use super::RuntimeMode;
use super::starlark::evaluate_file;

pub trait IncrementalEngine {
    fn runtime_mode(&self) -> RuntimeMode;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OneShotIncrementalEngine;

impl IncrementalEngine for OneShotIncrementalEngine {
    fn runtime_mode(&self) -> RuntimeMode {
        RuntimeMode::OneShot
    }
}

/// The result of evaluating the root Starlark files for one workspace.
///
/// This is intentionally just the Stage 2 runtime boundary. Stage 4 owns
/// Bazel file loading and Stage 5 owns the full `MODULE.bazel` global surface.
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct WorkspaceEvaluation {
    pub module: EvaluatedFile,
    pub build: EvaluatedFile,
}

/// Stage 4 package-loading evidence attached to a requested target pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedPackageEvaluation {
    pub target_pattern: String,
    pub package: LoadedPackage,
    pub analysis: Option<AnalysisResult>,
}

/// The V2 runtime result after the first configured-rule analysis packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBuildEvaluation {
    pub workspace: WorkspaceEvaluation,
    pub packages: Vec<RequestedPackageEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub struct EvaluatedFile {
    pub path: String,
    pub error: Option<String>,
}

impl EvaluatedFile {
    fn success(path: &Path) -> Self {
        Self {
            path: path.display().to_string(),
            error: None,
        }
    }

    fn failure(path: &Path, error: impl fmt::Display) -> Self {
        Self {
            path: path.display().to_string(),
            error: Some(error.to_string()),
        }
    }
}

impl WorkspaceEvaluation {
    fn into_result(self) -> anyhow::Result<Self> {
        for file in [&self.module, &self.build] {
            if let Some(error) = &file.error {
                anyhow::bail!("failed to evaluate {}: {error}", file.path);
            }
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Allocative)]
struct WorkspaceEvaluationKey {
    workspace: String,
}

impl fmt::Display for WorkspaceEvaluationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "workspace-evaluation:{}", self.workspace)
    }
}

#[async_trait]
impl Key for WorkspaceEvaluationKey {
    type Value = Arc<WorkspaceEvaluation>;

    async fn compute(
        &self,
        _ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        Arc::new(evaluate_workspace_files(Path::new(&self.workspace)))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }
}

fn evaluate_workspace_files(workspace: &Path) -> WorkspaceEvaluation {
    let module_path = workspace.join("MODULE.bazel");
    let build_path = workspace.join("BUILD.bazel");
    WorkspaceEvaluation {
        module: evaluate_workspace_file(&module_path, true),
        build: evaluate_workspace_file(&build_path, false),
    }
}

fn evaluate_workspace_file(path: &Path, is_module: bool) -> EvaluatedFile {
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => return EvaluatedFile::failure(path, error),
    };
    match evaluate_file(path, &source, is_module) {
        Ok(()) => EvaluatedFile::success(path),
        Err(error) => EvaluatedFile::failure(path, error),
    }
}

/// Open a real DICE transaction and evaluate the root module and package file
/// through the retained starlark-rust evaluator.
pub fn evaluate_workspace(workspace: impl Into<PathBuf>) -> anyhow::Result<WorkspaceEvaluation> {
    let workspace = workspace.into();
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("canonicalizing workspace {}", workspace.display()))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("creating one-shot DICE runtime")?;
    runtime.block_on(async move {
        let dice = Dice::builder().build(DetectCycles::Enabled);
        let mut transaction = dice.updater().commit().await;
        let evaluation = transaction
            .compute(&WorkspaceEvaluationKey {
                workspace: workspace.display().to_string(),
            })
            .await
            .context("computing root workspace evaluation through DICE")?;
        Arc::unwrap_or_clone(evaluation).into_result()
    })
}

/// Evaluate root files and each requested root-repository BUILD package.
///
/// Rule analysis remains outside this boundary, but package loading uses the
/// Stage 4 DICE-owned `.bzl` graph instead of a CLI-side parser shortcut.
pub fn evaluate_workspace_targets(
    workspace: impl Into<PathBuf>,
    targets: &[TargetPattern],
) -> anyhow::Result<WorkspaceBuildEvaluation> {
    let workspace = workspace.into();
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("canonicalizing workspace {}", workspace.display()))?;
    let workspace_evaluation = evaluate_workspace(workspace.clone())?;
    let package_evaluator = BzlModuleEvaluator::new(&workspace)?;
    let mut packages = Vec::with_capacity(targets.len());
    for target in targets {
        let package = package_path_for_target(&workspace, target)?;
        let loaded_package = package_evaluator.evaluate_package(package)?;
        let analysis = analysis_for_target(target, &loaded_package)?;
        packages.push(RequestedPackageEvaluation {
            target_pattern: target.to_string(),
            package: loaded_package,
            analysis,
        });
    }
    Ok(WorkspaceBuildEvaluation {
        workspace: workspace_evaluation,
        packages,
    })
}

fn analysis_for_target(
    target: &TargetPattern,
    package: &LoadedPackage,
) -> anyhow::Result<Option<AnalysisResult>> {
    let TargetPattern::Single(label) = target else {
        return Ok(None);
    };
    let target_name = label.target().as_str();
    let Some(package_target) = package
        .targets
        .iter()
        .find(|candidate| candidate.name == target_name)
    else {
        anyhow::bail!(
            "target `{target}` was not found in {}",
            package.build_file.display()
        );
    };
    if !matches!(
        package_target.kind,
        slug_loading_v2::PackageTargetKind::StarlarkRule(_)
    ) {
        return Ok(None);
    }
    let canonical = CanonicalLabel::parse(&format!(
        "@@//{}:{}",
        label.package().as_str(),
        label.target().as_str()
    ))
    .map_err(anyhow::Error::msg)?;
    let key = ConfiguredTargetKey::new(
        canonical,
        ConfigurationKey::target("first-build").map_err(anyhow::Error::msg)?,
    );
    analyze_loaded_rule(package, target_name, key, label.package().as_str())
        .map(Some)
        .map_err(anyhow::Error::msg)
}

fn package_path_for_target(workspace: &Path, target: &TargetPattern) -> anyhow::Result<PathBuf> {
    let (repo, package) = match target {
        TargetPattern::Single(label) => (label.repo(), label.package()),
        TargetPattern::PackageAll { repo, package } => (repo, package),
        TargetPattern::Recursive { .. } => {
            anyhow::bail!(
                "recursive target patterns are not supported before Stage 6 analysis: {target}"
            );
        }
    };
    if !repo.is_root() {
        anyhow::bail!(
            "external repository target patterns are not supported before Stage 5 repository mapping: {target}"
        );
    }
    Ok(workspace.join(package.as_str()))
}
