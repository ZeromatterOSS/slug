/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file or the Apache-License, Version 2.0 found in the
 * LICENSE-APACHE file in the root directory of this source tree. You may
 * select, at your option, one of the above-listed licenses.
 */

//! Same-daemon DICE invalidation for the load-invalidation gate clause.
//!
//! The daemon retains a [`BzlModuleEvaluator`] across builds. Before each
//! build it rescans the workspace for `.bzl` and `BUILD.bazel` files, compares
//! their SHA-256 digests to the previous build, and calls
//! [`BzlModuleEvaluator::invalidate_path`] / [`BzlModuleEvaluator::invalidate_package`]
//! for every changed path. The DICE graph then replays only the affected
//! computations.

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use sha2::Digest;
use sha2::Sha256;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::WorkspaceEvaluation;
use slug_core_v2::runtime::evaluate_packages_with;
use slug_core_v2::runtime::evaluate_workspace;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::BzlModuleEvaluator;
use slug_reapi_v2::RemoteConfig;
use slug_reapi_v2::RemoteMode;

use crate::reapi::run_reapi_build;

/// The retained daemon state: a DICE-backed `.bzl`/BUILD evaluator and a cache
/// of file digests from the previous build.
pub struct Daemon {
    workspace: PathBuf,
    evaluator: BzlModuleEvaluator,
    file_digests: HashMap<PathBuf, String>,
}

impl Daemon {
    /// Create a new daemon for the given workspace. The first build will have
    /// an empty digest cache, so every file is treated as new (no invalidation
    /// needed on the first build).
    pub fn new(workspace: impl AsRef<Path>) -> anyhow::Result<Self> {
        let workspace = workspace.as_ref().to_path_buf();
        let evaluator = BzlModuleEvaluator::new(&workspace)?;
        Ok(Self {
            workspace,
            evaluator,
            file_digests: HashMap::new(),
        })
    }

    /// Detect changed `.bzl` and `BUILD.bazel` files since the previous build
    /// and invalidate them through the DICE graph. Returns the number of
    /// invalidated paths.
    pub fn invalidate_changed(&mut self) -> anyhow::Result<usize> {
        let current = scan_workspace_files(&self.workspace)?;
        let mut invalidated = 0;
        for (path, digest) in &current {
            let changed = match self.file_digests.get(path) {
                Some(prev) => prev != digest,
                None => false,
            };
            if changed {
                if is_build_file(path) {
                    if let Some(package) = path.parent() {
                        self.evaluator.invalidate_package(package)?;
                    }
                } else {
                    self.evaluator.invalidate_path(path)?;
                }
                invalidated += 1;
            }
        }
        self.file_digests = current;
        Ok(invalidated)
    }

    /// Run one build: invalidate changed files, evaluate packages, analyze
    /// targets, and (if REAPI execute mode) execute actions. Returns the JSON
    /// evidence line for stderr.
    pub fn build(
        &mut self,
        targets: &[TargetPattern],
        remote: &RemoteConfig,
        argv: &[String],
    ) -> BuildResult {
        let invalidated = match self.invalidate_changed() {
            Ok(count) => count,
            Err(error) => {
                return BuildResult::error("build_runtime_error", &error.to_string());
            }
        };

        let workspace_evaluation: WorkspaceEvaluation = match evaluate_workspace(&self.workspace) {
            Ok(eval) => eval,
            Err(error) => {
                return BuildResult::error("build_runtime_error", &error.to_string());
            }
        };

        let evaluation = match evaluate_packages_with(
            &self.workspace,
            &workspace_evaluation,
            &self.evaluator,
            targets,
        ) {
            Ok(eval) => eval,
            Err(error) => {
                return BuildResult::error("build_runtime_error", &error.to_string());
            }
        };

        let analyzed_target_count = evaluation
            .packages
            .iter()
            .filter(|p| p.analysis.is_some())
            .count();
        let declared_action_count = evaluation
            .packages
            .iter()
            .filter_map(|p| p.analysis.as_ref())
            .map(|a| a.actions().len())
            .sum::<usize>();

        if remote.mode() == RemoteMode::Execute {
            let outcome = run_reapi_build(
                &self.workspace,
                &evaluation,
                analyzed_target_count,
                declared_action_count,
                remote,
                "daemon",
                invalidated,
            );
            return BuildResult {
                exit_code: outcome.exit_code,
                stderr: outcome.stderr,
                invalidated_files: invalidated,
            };
        }

        // Non-REAPI: analysis only (no local execution per non-negotiables).
        let argv_json = argv
            .iter()
            .map(|arg| format!("\"{}\"", json_escape(arg)))
            .collect::<Vec<_>>()
            .join(",");
        let completed_boundary = if analyzed_target_count == 0 {
            "dice_starlark_package_loading"
        } else {
            "dice_starlark_rule_analysis"
        };
        BuildResult {
            exit_code: 2,
            stderr: format!(
                "{{\"error\":\"analysis_not_implemented\",\"command\":\"build\",\"argv\":[{}],\"target_count\":{},\"loaded_package_count\":{},\"analyzed_target_count\":{},\"declared_action_count\":{},\"runtime_mode\":\"daemon\",\"invalidated_files\":{},\"completed_boundary\":\"{}\"}}",
                argv_json,
                targets.len(),
                evaluation.packages.len(),
                analyzed_target_count,
                declared_action_count,
                invalidated,
                completed_boundary,
            ),
            invalidated_files: invalidated,
        }
    }
}

/// Result of a daemon build request.
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub exit_code: i32,
    pub stderr: String,
    pub invalidated_files: usize,
}

impl BuildResult {
    fn error(kind: &str, message: &str) -> Self {
        Self {
            exit_code: 2,
            stderr: format!(
                "{{\"error\":\"{}\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":0}}",
                kind,
                json_escape(message),
            ),
            invalidated_files: 0,
        }
    }
}

/// Walk the workspace and hash every `.bzl` and `BUILD.bazel` / `BUILD` file.
fn scan_workspace_files(workspace: &Path) -> anyhow::Result<HashMap<PathBuf, String>> {
    let mut result = HashMap::new();
    let workspace = workspace.canonicalize().with_context(|| {
        format!(
            "canonicalizing workspace for file scan: {}",
            workspace.display()
        )
    })?;
    scan_dir(&workspace, &workspace, &mut result)?;
    Ok(result)
}

fn scan_dir(
    workspace: &Path,
    dir: &Path,
    out: &mut HashMap<PathBuf, String>,
) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            // Skip hidden directories and common build output dirs.
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "bazel-bin" || name == "bazel-out" {
                continue;
            }
            let _ = workspace;
            scan_dir(workspace, &path, out)?;
        } else if file_type.is_file() {
            if is_build_file(&path) || path.extension().is_some_and(|ext| ext == "bzl") {
                if let Ok(content) = std::fs::read(&path) {
                    let digest = hex_digest(&content);
                    out.insert(path, digest);
                }
            }
        }
    }
    Ok(())
}

fn is_build_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("BUILD.bazel") | Some("BUILD")
    )
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

mod reapi;
mod server;

pub use server::BuildRequest;
pub use server::BuildResponse;
pub use server::pid_path;
pub use server::send_build_request;
pub use server::send_shutdown;
pub use server::serve;
pub use server::socket_path;

#[cfg(test)]
mod tests;
