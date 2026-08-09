/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

mod configured_output;
mod demands;
pub mod dice;
mod events;
mod path_observation;
mod process_host;
pub mod reapi;
mod registry_io;
mod repository_io;
mod root_bootstrap;
pub mod starlark;

pub use configured_output::configured_output_root;
pub use dice::BuildCommandError;
pub use dice::BuildCommandEvaluation;
pub use dice::CqueryCommandError;
pub use dice::CqueryCommandEvaluation;
pub use dice::WorkspaceBuildEvaluation;
pub use dice::WorkspaceDirectoryObservation;
pub use dice::WorkspaceEvaluation;
pub use dice::WorkspaceFileObservation;
pub use dice::WorkspaceObservation;
pub use dice::WorkspaceRawFileObservation;
pub use dice::WorkspaceRevision;
pub use dice::WorkspaceRuntime;
pub use dice::evaluate_workspace;
pub use dice::evaluate_workspace_targets;
pub use dice::evaluate_workspace_targets_with_bzlmod_inputs;
pub use dice::observe_workspace;
pub use dice::observe_workspace_files;
pub use events::AcceptedCommand;
pub use events::CommandOutput;
pub use events::PublishedCommand;
pub use events::TerminalOutput;
pub use process_host::ProcessHostOwner;
pub use slug_query_v2::QueryError;
pub use slug_query_v2::QueryOutputCompletion;

/// One-shot typed build command. Source preparation observes only paths
/// demanded by the retained command root.
pub fn evaluate_workspace_build_command_with_bzlmod_inputs(
    workspace: &std::path::Path,
    targets: &[slug_identity_v2::TargetPattern],
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    root_string_setting: Option<&str>,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<BuildCommandEvaluation, BuildCommandError>>>,
    BuildCommandError,
> {
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(BuildCommandError::infrastructure)?;
    runtime.build_command_with_bzlmod_inputs(
        targets,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        root_string_setting,
    )
}

pub fn evaluate_workspace_cquery_command_with_bzlmod_inputs(
    workspace: &std::path::Path,
    expression: &str,
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    root_string_setting: Option<&str>,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>,
    CqueryCommandError,
> {
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(CqueryCommandError::infrastructure)?;
    runtime.cquery_command_with_bzlmod_inputs(
        expression,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        root_string_setting,
    )
}

/// One-shot typed query command. Source preparation observes only paths
/// demanded by the retained command root.
pub fn evaluate_workspace_query_command_with_policy_and_bzlmod_inputs_and_output_completion(
    workspace: &std::path::Path,
    expression: &str,
    order: slug_query_v2::QueryOrder,
    policy: slug_query_v2::QueryPolicy,
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    completion: slug_query_v2::QueryOutputCompletion,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError>>>,
    slug_query_v2::QueryError,
> {
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(|error| slug_query_v2::QueryError::evaluation(error.to_string()))?;
    runtime.query_command_with_policy_and_bzlmod_inputs_and_output_completion(
        expression,
        order,
        policy,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        completion,
    )
}

/// One-shot convenience that enters the identical retained-runtime query
/// method used by the daemon.
pub fn evaluate_workspace_query(
    workspace: &std::path::Path,
    expression: &str,
    order: slug_query_v2::QueryOrder,
) -> Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError> {
    evaluate_workspace_query_with_policy(
        workspace,
        expression,
        order,
        slug_query_v2::QueryPolicy::default(),
    )
}

pub fn evaluate_workspace_query_with_policy(
    workspace: &std::path::Path,
    expression: &str,
    order: slug_query_v2::QueryOrder,
    policy: slug_query_v2::QueryPolicy,
) -> Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError> {
    evaluate_workspace_query_with_policy_and_bzlmod_inputs(
        workspace,
        expression,
        order,
        policy,
        slug_bzlmod_v2::BzlmodCommandPolicyKey::from_flags(None, false)
            .expect("default bzlmod policy"),
        slug_bzlmod_v2::BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
            .expect("default bzlmod environment policy"),
        slug_bzlmod_v2::LockfileMode::Update,
        &[],
    )
}

pub fn evaluate_workspace_query_with_policy_and_bzlmod_inputs(
    workspace: &std::path::Path,
    expression: &str,
    order: slug_query_v2::QueryOrder,
    policy: slug_query_v2::QueryPolicy,
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
) -> Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError> {
    evaluate_workspace_query_with_policy_and_bzlmod_inputs_and_output_completion(
        workspace,
        expression,
        order,
        policy,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        slug_query_v2::QueryOutputCompletion::Standard,
    )
}

pub fn evaluate_workspace_query_with_policy_and_bzlmod_inputs_and_output_completion(
    workspace: &std::path::Path,
    expression: &str,
    order: slug_query_v2::QueryOrder,
    policy: slug_query_v2::QueryPolicy,
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    completion: slug_query_v2::QueryOutputCompletion,
) -> Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError> {
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(|error| slug_query_v2::QueryError::evaluation(error.to_string()))?;
    let observations = observe_workspace(workspace)
        .map_err(|error| slug_query_v2::QueryError::evaluation(error.to_string()))?;
    runtime.query_observations_with_policy_and_bzlmod_inputs_and_output_completion(
        observations,
        expression,
        order,
        policy,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        completion,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    OneShot,
    Daemon,
}

impl RuntimeMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneShot => "one-shot",
            Self::Daemon => "daemon",
        }
    }
}
