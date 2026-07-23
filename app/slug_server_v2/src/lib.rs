/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file or the Apache-License, Version 2.0 found in the
 * LICENSE-APACHE file in the root directory of this source tree. You may
 * select, at your option, one of the above-listed licenses.
 */

//! Same-daemon workspace runtime requests.
//!
//! The daemon owns one [`WorkspaceRuntime`]. A filesystem observation adapter
//! supplies complete present, absent, and failed-read values before each
//! request; DICE, not the adapter, determines semantic reuse.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::WorkspaceObservation;
use slug_core_v2::runtime::WorkspaceRuntime;
use slug_core_v2::runtime::observe_workspace;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryPolicy;
use slug_reapi_v2::RemoteConfig;
use slug_reapi_v2::RemoteMode;

use crate::reapi::run_reapi_build;

/// The retained daemon state: one workspace runtime and a non-semantic
/// observation adapter. The adapter's previous values are used only to report
/// the compatibility metric; every observation is injected into DICE.
pub struct Daemon {
    workspace: PathBuf,
    runtime: WorkspaceRuntime,
    observations: FilesystemObservationAdapter,
}

impl Daemon {
    /// Create a new daemon for the given workspace. The first build will have
    /// an empty digest cache, so every file is treated as new (no invalidation
    /// needed on the first build).
    pub fn new(workspace: impl AsRef<Path>) -> anyhow::Result<Self> {
        let runtime = WorkspaceRuntime::new(workspace.as_ref().to_path_buf())?;
        let workspace = runtime.workspace().to_path_buf();
        Ok(Self {
            workspace,
            runtime,
            observations: FilesystemObservationAdapter::default(),
        })
    }

    /// Run one build: observe files, inject one DICE batch, evaluate packages, analyze
    /// targets, and (if REAPI execute mode) execute actions. Returns the JSON
    /// evidence line for stderr.
    pub fn build(
        &mut self,
        targets: &[TargetPattern],
        remote: &RemoteConfig,
        argv: &[String],
    ) -> BuildResult {
        let (observations, invalidated) = match self.observations.observe(&self.workspace) {
            Ok(observations) => observations,
            Err(error) => {
                return BuildResult::error("build_runtime_error", &error.to_string());
            }
        };
        let evaluation = match self.runtime.evaluate_observations(observations, targets) {
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
                stdout: String::new(),
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
            stdout: String::new(),
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

    /// Run one loading query against the same retained runtime and observation
    /// adapter used by builds.
    pub fn query(&mut self, expression: &str, order: QueryOrder) -> QueryResult {
        self.query_with_policy(expression, order, QueryPolicy::default())
    }

    pub fn query_with_policy(
        &mut self,
        expression: &str,
        order: QueryOrder,
        policy: QueryPolicy,
    ) -> QueryResult {
        self.query_with_output_and_policy(expression, order, "text", true, policy)
    }

    /// Output selection formats the retained query result only. It does not
    /// create another DICE transaction or evaluate the expression again.
    pub fn query_with_output(
        &mut self,
        expression: &str,
        order: QueryOrder,
        output_format: &str,
        graph_factored: bool,
    ) -> QueryResult {
        self.query_with_output_and_policy(
            expression,
            order,
            output_format,
            graph_factored,
            QueryPolicy::default(),
        )
    }

    pub fn query_with_output_and_policy(
        &mut self,
        expression: &str,
        order: QueryOrder,
        output_format: &str,
        graph_factored: bool,
        policy: QueryPolicy,
    ) -> QueryResult {
        let (observations, invalidated) = match self.observations.observe(&self.workspace) {
            Ok(observations) => observations,
            Err(error) => {
                return QueryResult::error(7, &error.to_string());
            }
        };
        match self
            .runtime
            .query_observations_with_policy(observations, expression, order, policy)
        {
            Ok(output) => {
                let stdout = match output_format {
                    "text" => output.stdout(),
                    "graph" => output.graph_stdout(graph_factored, order.is_full()),
                    other => {
                        return QueryResult::error(
                            2,
                            &format!("output format '{other}' is not supported by loading query"),
                        );
                    }
                };
                QueryResult {
                    exit_code: 0,
                    stdout,
                    stderr: String::new(),
                    invalidated_files: invalidated,
                }
            }
            Err(error) => QueryResult {
                exit_code: error.exit_code,
                stdout: String::new(),
                stderr: format!(
                    "{{\"error\":\"query_error\",\"command\":\"query\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":{}}}",
                    json_escape(&query_error_message(&error)),
                    invalidated,
                ),
                invalidated_files: invalidated,
            },
        }
    }
}

fn query_error_message(error: &slug_query_v2::QueryError) -> String {
    let mut message = error.to_string();
    if error.needs_evaluation_context() {
        message.push_str("\nEvaluation of query");
    }
    message
}

/// Result of a daemon build request.
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub invalidated_files: usize,
}

impl BuildResult {
    fn error(kind: &str, message: &str) -> Self {
        Self {
            exit_code: 2,
            stdout: String::new(),
            stderr: format!(
                "{{\"error\":\"{}\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":0}}",
                kind,
                json_escape(message),
            ),
            invalidated_files: 0,
        }
    }
}

/// Result of a daemon query request.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub invalidated_files: usize,
}

impl QueryResult {
    fn error(exit_code: i32, message: &str) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr: format!(
                "{{\"error\":\"query_runtime_error\",\"command\":\"query\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":0}}",
                json_escape(message),
            ),
            invalidated_files: 0,
        }
    }
}

#[derive(Default)]
struct FilesystemObservationAdapter {
    previous: Option<BTreeMap<PathBuf, WorkspaceFileValue>>,
}

impl FilesystemObservationAdapter {
    fn observe(&mut self, workspace: &Path) -> anyhow::Result<(WorkspaceObservation, usize)> {
        let observations = observe_workspace(workspace)?;
        let current = observations
            .files
            .iter()
            .map(|observation| (observation.path.clone(), observation.value.clone()))
            .collect::<BTreeMap<_, _>>();
        let invalidated = self.previous.as_ref().map_or(0, |previous| {
            previous
                .iter()
                .filter(|(path, value)| current.get(*path) != Some(*value))
                .count()
                + current
                    .keys()
                    .filter(|path| !previous.contains_key(*path))
                    .count()
        });
        self.previous = Some(current);
        Ok((observations, invalidated))
    }
}

mod reapi;
mod server;

pub use server::BuildRequest;
pub use server::BuildResponse;
pub use server::DaemonRequest;
pub use server::DaemonResponse;
pub use server::QueryRequest;
pub use server::pid_path;
pub use server::send_build_request;
pub use server::send_query_request;
pub use server::send_shutdown;
pub use server::serve;
pub use server::socket_path;

#[cfg(test)]
mod tests;
