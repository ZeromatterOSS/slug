/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

mod demands;
pub mod dice;
mod events;
mod path_observation;
pub mod reapi;
mod registry_io;
mod repository_io;
pub mod starlark;

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
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf())
        .map_err(|error| slug_query_v2::QueryError::evaluation(error.to_string()))?;
    let observations = observe_workspace(workspace)
        .map_err(|error| slug_query_v2::QueryError::evaluation(error.to_string()))?;
    runtime.query_observations_with_policy_and_bzlmod_inputs(
        observations,
        expression,
        order,
        policy,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
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
