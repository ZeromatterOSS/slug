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
//! retains compatibility invalidation metrics and failure behavior; typed
//! build/query commands discover their semantic path inputs through DICE.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use slug_bzlmod_v2::BzlmodCommandPolicyKey;
use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
use slug_bzlmod_v2::LockfileMode;
use slug_core_v2::error::json_escape;
use slug_core_v2::runtime::ProcessHostOwner;
use slug_core_v2::runtime::TerminalOutput;
use slug_core_v2::runtime::WorkspaceObservation;
use slug_core_v2::runtime::WorkspaceRuntime;
use slug_core_v2::runtime::observe_workspace;
use slug_identity_v2::TargetPattern;
use slug_loading_v2::keys::WorkspaceFileValue;
use slug_query_v2::QueryOrder;
use slug_query_v2::QueryOutputCompletion;
use slug_query_v2::QueryPolicy;
use slug_reapi_v2::RemoteConfig;
use slug_reapi_v2::RemoteMode;

use crate::reapi::run_reapi_build;

/// The retained daemon state: one workspace runtime and a non-semantic
/// observation adapter. The adapter's previous values are used only to report
/// the compatibility metric.
pub struct Daemon {
    workspace: PathBuf,
    runtime: WorkspaceRuntime,
    observations: FilesystemObservationAdapter,
    #[cfg(test)]
    forwarded_bzlmod_inputs: Vec<crate::server::BzlmodRequestInputs>,
    #[cfg(test)]
    process_host_for_test: std::sync::Arc<ProcessHostOwner>,
}

impl Daemon {
    /// Create a new daemon for the given workspace. The first build will have
    /// an empty digest cache, so every file is treated as new (no invalidation
    /// needed on the first build).
    pub fn new(workspace: impl AsRef<Path>) -> anyhow::Result<Self> {
        let process_host = ProcessHostOwner::unsupported();
        let runtime =
            WorkspaceRuntime::new(workspace.as_ref().to_path_buf(), process_host.clone())?;
        let workspace = runtime.workspace().to_path_buf();
        Ok(Self {
            workspace,
            #[cfg(test)]
            process_host_for_test: process_host,
            runtime,
            observations: FilesystemObservationAdapter::default(),
            #[cfg(test)]
            forwarded_bzlmod_inputs: Vec::new(),
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
        self.build_with_bzlmod_inputs(
            targets,
            remote,
            argv,
            BzlmodCommandPolicyKey::from_flags(None, false).expect("default bzlmod policy"),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                .expect("default bzlmod environment policy"),
            LockfileMode::Update,
            Vec::new(),
        )
    }

    pub fn build_with_bzlmod_inputs(
        &mut self,
        targets: &[TargetPattern],
        remote: &RemoteConfig,
        argv: &[String],
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: Vec<String>,
    ) -> BuildResult {
        let (_metric_observations, invalidated) = match self.observations.observe(&self.workspace) {
            Ok(observations) => observations,
            Err(error) => {
                return BuildResult::error("build_runtime_error", &error.to_string());
            }
        };
        #[cfg(test)]
        self.forwarded_bzlmod_inputs.push(
            crate::server::BzlmodRequestInputs::from_normalized_with_registry_urls(
                &command_policy,
                &environment_policy,
                &lockfile_mode,
                &registry_urls,
            ),
        );
        let accepted = match self.runtime.build_command_with_bzlmod_inputs(
            targets,
            command_policy,
            environment_policy,
            lockfile_mode,
            &registry_urls,
        ) {
            Ok(accepted) => accepted,
            Err(error) => {
                return BuildResult::error_with_invalidated(
                    "build_runtime_error",
                    &error.to_string(),
                    invalidated,
                );
            }
        };
        let published = accepted
            .project(|terminal| match terminal.as_ref() {
                Err(error) => {
                    let (kind, exit_code) = error.terminal_error();
                    TerminalOutput::new(
                        exit_code,
                        String::new(),
                        build_error_json(kind, &error.to_string(), invalidated),
                    )
                }
                Ok(evaluation) => {
                    if evaluation.is_observed_exported_source() {
                        return TerminalOutput::new(
                            0,
                            String::new(),
                            format!(
                                "{{\"success\":true,\"command\":\"build\",\"target_count\":1,\"loaded_package_count\":1,\"analyzed_target_count\":0,\"declared_action_count\":0,\"runtime_mode\":\"daemon\",\"invalidated_files\":{invalidated},\"completed_boundary\":\"dice_exported_source_file\"}}\n"
                            ),
                        );
                    }
                    let analyzed_target_count = evaluation.analyzed_target_count();
                    let declared_action_count = evaluation.declared_action_count();
                    if remote.mode() == RemoteMode::Execute {
                        let outcome = run_reapi_build(
                            &self.workspace,
                            evaluation,
                            analyzed_target_count,
                            declared_action_count,
                            remote,
                            "daemon",
                            invalidated,
                        );
                        TerminalOutput::new(outcome.exit_code, String::new(), outcome.stderr)
                    } else {
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
                        TerminalOutput::new(
                            2,
                            String::new(),
                            format!(
                                "{{\"error\":\"analysis_not_implemented\",\"command\":\"build\",\"argv\":[{}],\"target_count\":{},\"loaded_package_count\":{},\"analyzed_target_count\":{},\"declared_action_count\":{},\"runtime_mode\":\"daemon\",\"invalidated_files\":{},\"completed_boundary\":\"{}\"}}\n",
                                argv_json,
                                targets.len(),
                                evaluation.loaded_package_count(),
                                analyzed_target_count,
                                declared_action_count,
                                invalidated,
                                completed_boundary,
                            ),
                        )
                    }
                }
            })
            .publish();
        let (_terminal, exit_code, stdout, stderr) = published.into_parts();
        BuildResult {
            exit_code,
            stdout,
            stderr,
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
        self.query_with_output_policy_and_bzlmod_inputs(
            expression,
            order,
            output_format,
            graph_factored,
            policy,
            BzlmodCommandPolicyKey::from_flags(None, false).expect("default bzlmod policy"),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                .expect("default bzlmod environment policy"),
            LockfileMode::Update,
            Vec::new(),
        )
    }

    pub fn query_with_output_policy_and_bzlmod_inputs(
        &mut self,
        expression: &str,
        order: QueryOrder,
        output_format: &str,
        graph_factored: bool,
        policy: QueryPolicy,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: Vec<String>,
    ) -> QueryResult {
        let completion = match output_format {
            "text" | "label" | "graph" | "package" => QueryOutputCompletion::Standard,
            "label_kind" => QueryOutputCompletion::LabelKind,
            other => {
                return QueryResult::error(
                    2,
                    &format!("output format '{other}' is not supported by loading query"),
                );
            }
        };
        let (_metric_observations, invalidated) = match self.observations.observe(&self.workspace) {
            Ok(observations) => observations,
            Err(error) => {
                return QueryResult::error(7, &error.to_string());
            }
        };
        #[cfg(test)]
        self.forwarded_bzlmod_inputs.push(
            crate::server::BzlmodRequestInputs::from_normalized_with_registry_urls(
                &command_policy,
                &environment_policy,
                &lockfile_mode,
                &registry_urls,
            ),
        );
        let accepted = match self
            .runtime
            .query_command_with_policy_and_bzlmod_inputs_and_output_completion(
                expression,
                order,
                policy,
                command_policy,
                environment_policy,
                lockfile_mode,
                &registry_urls,
                completion,
            ) {
            Ok(accepted) => accepted,
            Err(error) => return QueryResult::query_error(&error, invalidated),
        };
        let published = accepted
            .project(|terminal| match terminal.as_ref() {
                Ok(output) => {
                    let stdout = match output_format {
                        "text" | "label" => output.stdout(),
                        "graph" => output.graph_stdout(graph_factored, order.is_full()),
                        "label_kind" => output.label_kind_stdout(),
                        "package" => output.package_stdout(),
                        _ => unreachable!("output format was validated before evaluation"),
                    };
                    TerminalOutput::new(0, stdout, String::new())
                }
                Err(error) => TerminalOutput::new(
                    error.exit_code,
                    String::new(),
                    query_error_json(error, invalidated),
                ),
            })
            .publish();
        let (_terminal, exit_code, stdout, stderr) = published.into_parts();
        QueryResult {
            exit_code,
            stdout,
            stderr,
            invalidated_files: invalidated,
        }
    }

    pub fn cquery_starlark_label_with_bzlmod_inputs(
        &mut self,
        target: &TargetPattern,
        command_policy: BzlmodCommandPolicyKey,
        environment_policy: BzlmodEnvironmentPolicyKey,
        lockfile_mode: LockfileMode,
        registry_urls: Vec<String>,
    ) -> QueryResult {
        let (_metric_observations, invalidated) = match self.observations.observe(&self.workspace) {
            Ok(observations) => observations,
            Err(error) => return cquery_error_result(&error.to_string(), 0),
        };
        #[cfg(test)]
        self.forwarded_bzlmod_inputs.push(
            crate::server::BzlmodRequestInputs::from_normalized_with_registry_urls(
                &command_policy,
                &environment_policy,
                &lockfile_mode,
                &registry_urls,
            ),
        );
        let accepted = match self
            .runtime
            .cquery_starlark_label_command_with_bzlmod_inputs(
                target,
                command_policy,
                environment_policy,
                lockfile_mode,
                &registry_urls,
            ) {
            Ok(accepted) => accepted,
            Err(error) => return cquery_error_result_for_terminal(&error, invalidated),
        };
        let published = accepted
            .project(|terminal| match terminal.as_ref() {
                Ok(evaluation) => {
                    TerminalOutput::new(0, evaluation.starlark_label_stdout(), String::new())
                }
                Err(error) => match error.missing_stderr() {
                    Some(stderr) => TerminalOutput::new(1, String::new(), stderr),
                    None => {
                        TerminalOutput::new(2, String::new(), cquery_error_json(error, invalidated))
                    }
                },
            })
            .publish();
        let (_terminal, exit_code, stdout, stderr) = published.into_parts();
        QueryResult {
            exit_code,
            stdout,
            stderr,
            invalidated_files: invalidated,
        }
    }

    #[cfg(test)]
    fn take_forwarded_bzlmod_inputs_for_test(&mut self) -> Vec<crate::server::BzlmodRequestInputs> {
        std::mem::take(&mut self.forwarded_bzlmod_inputs)
    }
}

fn query_error_message(error: &slug_query_v2::QueryError) -> String {
    let mut message = error.to_string();
    if error.needs_evaluation_context() {
        message.push_str("\nEvaluation of query");
    }
    message
}

fn query_error_json(error: &slug_query_v2::QueryError, invalidated_files: usize) -> String {
    format!(
        "{{\"error\":\"{}\",\"command\":\"query\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":{invalidated_files}}}\n",
        error.error_kind(),
        json_escape(&query_error_message(error)),
    )
}

fn cquery_error_json(
    error: &slug_core_v2::runtime::CqueryCommandError,
    invalidated_files: usize,
) -> String {
    format!(
        "{{\"error\":\"cquery_runtime_error\",\"command\":\"cquery\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":{invalidated_files}}}\n",
        json_escape(&error.to_string()),
    )
}

/// Result of a daemon build request.
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub invalidated_files: usize,
}

fn build_error_json(kind: &str, message: &str, invalidated_files: usize) -> String {
    format!(
        "{{\"error\":\"{}\",\"command\":\"build\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":{invalidated_files}}}\n",
        kind,
        json_escape(message),
    )
}

impl BuildResult {
    fn error(kind: &str, message: &str) -> Self {
        Self::error_with_invalidated(kind, message, 0)
    }

    fn error_with_invalidated(kind: &str, message: &str, invalidated_files: usize) -> Self {
        Self {
            exit_code: 2,
            stdout: String::new(),
            stderr: build_error_json(kind, message, invalidated_files),
            invalidated_files,
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

fn cquery_error_result(message: &str, invalidated_files: usize) -> QueryResult {
    QueryResult {
        exit_code: 2,
        stdout: String::new(),
        stderr: format!(
            "{{\"error\":\"cquery_runtime_error\",\"command\":\"cquery\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":{invalidated_files}}}\n",
            json_escape(message),
        ),
        invalidated_files,
    }
}

fn cquery_error_result_for_terminal(
    error: &slug_core_v2::runtime::CqueryCommandError,
    invalidated_files: usize,
) -> QueryResult {
    match error.missing_stderr() {
        Some(stderr) => QueryResult {
            exit_code: 1,
            stdout: String::new(),
            stderr,
            invalidated_files,
        },
        None => cquery_error_result(&error.to_string(), invalidated_files),
    }
}

impl QueryResult {
    fn error(exit_code: i32, message: &str) -> Self {
        Self {
            exit_code,
            stdout: String::new(),
            stderr: format!(
                "{{\"error\":\"query_runtime_error\",\"command\":\"query\",\"message\":\"{}\",\"runtime_mode\":\"daemon\",\"invalidated_files\":0}}\n",
                json_escape(message),
            ),
            invalidated_files: 0,
        }
    }

    fn query_error(error: &slug_query_v2::QueryError, invalidated_files: usize) -> Self {
        Self {
            exit_code: error.exit_code,
            stdout: String::new(),
            stderr: query_error_json(error, invalidated_files),
            invalidated_files,
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
pub use server::BzlmodRequestInputs;
pub use server::CqueryRequest;
pub use server::DaemonRequest;
pub use server::DaemonResponse;
pub use server::QueryRequest;
pub use server::pid_path;
pub use server::send_build_request;
pub use server::send_cquery_request;
pub use server::send_query_request;
pub use server::send_shutdown;
pub use server::serve;
pub use server::socket_path;

#[cfg(test)]
mod tests;
