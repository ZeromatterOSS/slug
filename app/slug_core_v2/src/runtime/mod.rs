/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

mod action_output_staging;
mod action_prerequisites;
pub use action_output_staging::ActionOutputStaging;
pub use action_output_staging::PlannedActionOutputStaging;
pub use action_output_staging::PublishedActionOutputs;
pub use action_output_staging::PublishedPlannedActionOutputs;
pub use action_prerequisites::ActionPrerequisitePlan;
pub use action_prerequisites::PlannedAction;
pub use action_prerequisites::PlannedActionInput;
pub use action_prerequisites::RequestedActionPrerequisitePlan;
mod archive_distdir;
pub use archive_distdir::ArchiveDistdirPolicy;
mod aquery_text;
mod build_info_aquery_text;
mod configured_action_closure;
mod configured_output;
mod cpp_compile_aquery_text;
mod demands;
pub mod dice;
mod events;
mod file_write_aquery_text;
mod file_write_identity;
mod generated_repository_definition;
mod input_image;
#[cfg(all(test, target_os = "linux"))]
mod materialization_diagnostic;
mod native_identity;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod native_output_file;
mod native_source_access;
pub use native_source_access::NativeActionChainInputs;
pub use native_source_access::NativeAsyncSourceReader;
pub use native_source_access::NativeSourceActionInputs;
pub use native_source_access::NativeSourceReader;
mod path_observation;
#[cfg(feature = "native-probe-observer")]
pub mod probe_observer;
mod process_host;
pub mod reapi;
mod registry_io;
mod repository_archive;
mod repository_archive_checksum;
mod repository_archive_file;
mod repository_archive_http;
mod repository_archive_patch;
mod repository_archive_realize;
mod repository_filesystem_progress;
mod repository_generated_archive;
mod repository_host_input;
mod repository_io;
#[cfg(target_os = "linux")]
#[allow(dead_code)] // Inactive WP425 ownership; native integration remains gated.
mod repository_physical_root;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
mod repository_process_policy;
#[cfg(target_os = "linux")]
#[allow(dead_code)] // Inactive until the owning general runner is integrated.
mod repository_process_wire;
#[cfg(target_os = "linux")]
#[allow(dead_code)] // Inactive WP425 primitives; owners are integrated separately.
mod repository_root_io;
#[cfg(test)]
mod repository_workspace_comparison_work_for_test;
mod request_revision;
mod root_apparent_repository_definition;
mod root_apparent_repository_route;
mod root_apparent_repository_source_input;
mod root_apparent_repository_source_observation;
mod root_apparent_repository_source_path_input;
mod root_bootstrap;
mod runfiles_support_aquery_text;
mod scoped_cycle_detector;
mod solib_aquery_text;
#[cfg(all(test, target_os = "linux"))]
mod source_attempt_context;
mod source_input;
mod spawn_query_inputs;
pub mod starlark;
mod symlink_action_equivalence;
mod symlink_aquery_text;
mod workspace_status_inputs;
#[allow(dead_code)]
mod workspace_status_publication_file;

pub use aquery_text::format_aquery_text_output_for_scope;
pub use aquery_text::format_aquery_text_output_for_scope_with_context;
pub use configured_output::configured_output_root;
pub use dice::ActionChainOutputTransport;
pub use dice::ActionChainResult;
pub use dice::ActionChainTransport;
pub use dice::BuildCommandError;
pub use dice::BuildCommandEvaluation;
pub use dice::CqueryCommandError;
pub use dice::CqueryCommandEvaluation;
pub use dice::PreparedActionChainInputs;
pub use dice::PreparedActionPlan;
pub use dice::PreparedArtifactSymlink;
pub use dice::PreparedRunfilesAction;
pub use dice::PreparedRunfilesManifests;
#[doc(hidden)]
pub use dice::PreparedSourceActionInputs;
#[cfg(all(
    feature = "repository-workspace-test-support",
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
pub use dice::RepositoryWorkspaceIntegrationTestResources;
#[cfg(all(
    feature = "repository-workspace-test-support",
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
pub use dice::RepositoryWorkspaceIntegrationTestSnapshot;
pub use dice::RequestedActionResult;
pub use dice::RequestedArtifacts;
pub use dice::RequestedOutputGroup;
pub use dice::RequestedTargetArtifacts;
pub use dice::ResolvedFileWriteSemanticView;
pub use dice::ResolvedRunSemanticView;
pub use dice::SourceActionResult;
pub use dice::SourceActionTransport;
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
pub use file_write_aquery_text::format_file_write_aquery_text;
pub use file_write_aquery_text::format_file_write_aquery_text_output;
pub use file_write_aquery_text::format_file_write_aquery_text_output_for_scope;
pub use file_write_identity::FileWriteSemanticIdentity;
pub use process_host::ProcessHostOwner;
pub use slug_analysis_v2::ConfiguredNodeKind;
pub use slug_bzlmod_v2::BzlmodCommandPolicyKey;
pub use slug_bzlmod_v2::BzlmodEnvironmentPolicyKey;
pub use slug_bzlmod_v2::LockfileMode;
pub use slug_identity_v2::TargetPattern;
pub use slug_query_v2::QueryError;
pub use slug_query_v2::QueryOutputCompletion;
#[doc(hidden)]
pub use source_input::ObservedSourceArtifactInput;
#[doc(hidden)]
pub use source_input::SourceArtifactInput;
#[doc(hidden)]
pub use source_input::SourceArtifactInputError;
#[doc(hidden)]
pub use source_input::SourceArtifactInputObservationKey;
#[doc(hidden)]
pub use source_input::SourceArtifactInputOutcome;

#[cfg(all(test, feature = "native-probe-observer"))]
static PANIC_AFTER_OBSERVER_ATTACH: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg(all(test, feature = "native-probe-observer"))]
fn arm_observer_api_panic_for_test() {
    PANIC_AFTER_OBSERVER_ATTACH.store(true, std::sync::atomic::Ordering::Release);
}

/// One-shot typed build command. Source preparation observes only paths
/// demanded by the retained command root.
pub fn evaluate_workspace_build_command_with_bzlmod_inputs(
    workspace: &std::path::Path,
    targets: &[slug_identity_v2::TargetPattern],
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    configuration_overlay: slug_configuration_v2::CommandConfigurationOverlay,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<BuildCommandEvaluation, BuildCommandError>>>,
    BuildCommandError,
> {
    #[cfg(feature = "native-probe-observer")]
    let observer = probe_observer::capture();
    #[cfg(feature = "native-probe-observer")]
    if let Some(observer) = observer.as_ref() {
        observer.enter(probe_observer::Phase::NativeApi);
    }
    #[cfg(feature = "native-probe-observer")]
    let _api_finished = observer
        .as_ref()
        .map(|observer| observer.deferred_exit(probe_observer::Phase::NativeApi));
    #[cfg(feature = "native-probe-observer")]
    let mut teardown_finished = None;
    #[cfg(feature = "native-probe-observer")]
    let construction = observer
        .as_ref()
        .map(|observer| observer.phase(probe_observer::Phase::RuntimeConstruction));
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(BuildCommandError::infrastructure)?;
    #[cfg(feature = "native-probe-observer")]
    let mut runtime = runtime;
    #[cfg(feature = "native-probe-observer")]
    if let Some(construction) = construction {
        construction.finish();
    }
    #[cfg(feature = "native-probe-observer")]
    if let Some(observer) = observer.as_ref() {
        runtime.attach_probe_observer(observer.clone());
        teardown_finished = Some(observer.deferred_exit(probe_observer::Phase::RuntimeTeardown));
    }
    #[cfg(feature = "native-probe-observer")]
    let _teardown_started = observer
        .as_ref()
        .map(|observer| observer.deferred_entry(probe_observer::Phase::RuntimeTeardown));
    #[cfg(all(test, feature = "native-probe-observer"))]
    if PANIC_AFTER_OBSERVER_ATTACH.swap(false, std::sync::atomic::Ordering::AcqRel) {
        panic!("injected observer API unwind");
    }
    let result = runtime.build_command_with_bzlmod_inputs(
        targets,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        configuration_overlay,
    );
    #[cfg(feature = "native-probe-observer")]
    std::hint::black_box(&mut teardown_finished);
    result
}

pub fn evaluate_workspace_build_command_with_repository_environment(
    workspace: &std::path::Path,
    targets: &[slug_identity_v2::TargetPattern],
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
    configuration_overlay: slug_configuration_v2::CommandConfigurationOverlay,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<BuildCommandEvaluation, BuildCommandError>>>,
    BuildCommandError,
> {
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(BuildCommandError::infrastructure)?;
    runtime.build_command_with_repository_environment(
        targets,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        repository_environment,
        configuration_overlay,
    )
}

pub fn evaluate_workspace_cquery_command_with_bzlmod_inputs(
    workspace: &std::path::Path,
    expression: &str,
    include_implicit: bool,
    include_tool: bool,
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    configuration_overlay: slug_configuration_v2::CommandConfigurationOverlay,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>,
    CqueryCommandError,
> {
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(CqueryCommandError::infrastructure)?;
    runtime.cquery_command_with_bzlmod_inputs(
        expression,
        include_implicit,
        include_tool,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        configuration_overlay,
    )
}

pub fn evaluate_workspace_cquery_command_with_repository_environment(
    workspace: &std::path::Path,
    expression: &str,
    include_implicit: bool,
    include_tool: bool,
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
    configuration_overlay: slug_configuration_v2::CommandConfigurationOverlay,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>,
    CqueryCommandError,
> {
    evaluate_workspace_cquery_command_with_repository_environment_and_distdirs(
        workspace,
        expression,
        include_implicit,
        include_tool,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        repository_environment,
        configuration_overlay,
        ArchiveDistdirPolicy::default(),
    )
}

pub fn evaluate_workspace_cquery_command_with_repository_environment_and_distdirs(
    workspace: &std::path::Path,
    expression: &str,
    include_implicit: bool,
    include_tool: bool,
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
    configuration_overlay: slug_configuration_v2::CommandConfigurationOverlay,
    archive_distdirs: ArchiveDistdirPolicy,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<CqueryCommandEvaluation, CqueryCommandError>>>,
    CqueryCommandError,
> {
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(CqueryCommandError::infrastructure)?;
    runtime.cquery_command_with_repository_environment_and_distdirs(
        expression,
        include_implicit,
        include_tool,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        repository_environment,
        configuration_overlay,
        archive_distdirs,
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

pub fn evaluate_workspace_query_command_with_repository_environment(
    workspace: &std::path::Path,
    expression: &str,
    order: slug_query_v2::QueryOrder,
    policy: slug_query_v2::QueryPolicy,
    command_policy: slug_bzlmod_v2::BzlmodCommandPolicyKey,
    environment_policy: slug_bzlmod_v2::BzlmodEnvironmentPolicyKey,
    lockfile_mode: slug_bzlmod_v2::LockfileMode,
    registry_urls: &[String],
    repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot,
    completion: slug_query_v2::QueryOutputCompletion,
) -> Result<
    AcceptedCommand<std::sync::Arc<Result<slug_query_v2::QueryOutput, slug_query_v2::QueryError>>>,
    slug_query_v2::QueryError,
> {
    let runtime = WorkspaceRuntime::new(workspace.to_path_buf(), ProcessHostOwner::native())
        .map_err(|error| slug_query_v2::QueryError::evaluation(error.to_string()))?;
    runtime.query_command_with_repository_environment(
        expression,
        order,
        policy,
        command_policy,
        environment_policy,
        lockfile_mode,
        registry_urls,
        repository_environment,
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

#[cfg(test)]
mod source_observation_surface_tests {
    use std::sync::Arc;

    use dice::Key;
    use slug_identity_v2::ApparentRepoName;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationEpoch;

    use super::root_apparent_repository_source_observation::*;

    #[test]
    fn root_apparent_repository_source_observation_surface_is_sibling_usable() {
        let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
        let key = HostRootApparentRepositorySourceObservationKey::new(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        let observed_key = HostRootApparentRepositorySourceObservationObservationKey::new(
            workspace.clone(),
            ApparentRepoName::new("first").unwrap(),
            "pkg/file.bzl".into(),
        )
        .unwrap();
        assert_eq!(
            key.to_string(),
            "HostRootApparentRepositorySourceObservationKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\"), requested_path: \"pkg/file.bzl\" }"
        );
        assert_eq!(
            observed_key.to_string(),
            "observed-HostRootApparentRepositorySourceObservationKey { workspace: NormalizedAbsolutePath { path: \"/workspace\" }, apparent_repo: ApparentRepoName(\"first\"), requested_path: \"pkg/file.bzl\" }"
        );
        assert!(
            HostRootApparentRepositorySourceObservationKey::new(
                workspace.clone(),
                ApparentRepoName::root(),
                "x".into(),
            )
            .is_none()
        );
        assert!(
            HostRootApparentRepositorySourceObservationObservationKey::new(
                workspace,
                ApparentRepoName::root(),
                "x".into(),
            )
            .is_none()
        );

        fn inspect(
            _: &<HostRootApparentRepositorySourceObservationKey as Key>::Value,
            _: &<HostRootApparentRepositorySourceObservationObservationKey as Key>::Value,
            result: &HostRootApparentRepositorySourceObservationResult,
            observed: &ObservedHostRootApparentRepositorySourceObservation,
            _: &HostRootApparentRepositorySourceObservationError,
            _: &HostRootApparentRepositorySourceObservationObservationError,
        ) {
            let _: &Arc<HostRootApparentRepositorySourceObservationResult> = observed.result();
            let _: &PathObservationEpoch = observed.observations();
            if let Ok(certificate) = result {
                if let Some(view) = certificate.view() {
                    let _ = view.apparent_repo();
                    let _ = view.canonical_repo();
                    let _ = view.relative_path();
                    match view.disposition() {
                        HostRootApparentRepositorySourceObservationDispositionView::Main => {}
                        HostRootApparentRepositorySourceObservationDispositionView::Input {
                            input: _,
                            observation: _,
                        } => {}
                    }
                }
            }
        }
        let _: fn(_, _, _, _, _, _) = inspect;
    }
}

/// Runs the closed repository helper ABI before CLI/runtime initialization.
/// The marker grants no authority: the helper validates PID1, sealed plan,
/// anonymous control and owning-namespace capabilities before setup.
#[doc(hidden)]
pub fn repository_process_helper_main() -> Option<i32> {
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    {
        let mut args = std::env::args_os();
        let _ = args.next();
        if args.next().as_deref()
            == Some(std::ffi::OsStr::new(
                repository_process_wire::HELPER_ARGUMENT,
            ))
        {
            return Some(if args.next().is_none() {
                repository_physical_root::run_process_helper()
            } else {
                125
            });
        }
    }
    None
}
