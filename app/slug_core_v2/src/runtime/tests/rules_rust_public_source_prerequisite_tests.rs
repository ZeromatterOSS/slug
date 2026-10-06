use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::dice::source_service::SourceServiceScope;
use crate::runtime::dice::source_service::WorkspaceSourceServices;

fn phase(name: &str) {
    let status = fs::read_to_string("/proc/self/status").expect("reading bounded process status");
    let rss = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|line| line.split_whitespace().next())
        .and_then(|value| value.parse::<usize>().ok())
        .expect("process status has numeric RSS");
    println!("WP938_PHASE {name} {rss}");
}

fn build_kind(error: &BuildCommandError) -> &'static str {
    match &error.kind {
        BuildCommandErrorKind::RootAnchor(_) => "root_anchor",
        BuildCommandErrorKind::Package(_) => "package",
        BuildCommandErrorKind::RepositoryRoute(_) => "repository_route",
        BuildCommandErrorKind::CanonicalRepositoryRoute(_) => "canonical_repository_route",
        BuildCommandErrorKind::CanonicalRepositoryRouteObservation(_) => {
            "canonical_route_observation"
        }
        BuildCommandErrorKind::GeneratedRouteObservation { .. } => "generated_route_observation",
        BuildCommandErrorKind::RepositoryPackage(_) => "repository_package",
        BuildCommandErrorKind::RepositorySource(..) => "repository_source",
        BuildCommandErrorKind::RepositorySourceObservation(..) => "repository_source_observation",
        BuildCommandErrorKind::SourceMissing(..) => "source_missing",
        BuildCommandErrorKind::RootSource { .. } => "root_source",
        BuildCommandErrorKind::ExternalTargetKind => "external_target_kind",
        BuildCommandErrorKind::TargetNotFound { .. } => "target_not_found",
        BuildCommandErrorKind::Analysis(_) => "analysis",
        BuildCommandErrorKind::SourceCertified { .. } => "source_certified",
        BuildCommandErrorKind::SourceArtifactInput(_) => "source_artifact_input",
        BuildCommandErrorKind::ExternalRepository { .. } => "external_repository",
        BuildCommandErrorKind::RecursivePattern { .. } => "recursive_pattern",
        BuildCommandErrorKind::AllTargetsPattern { .. } => "all_targets_pattern",
        BuildCommandErrorKind::Infrastructure(_) => "infrastructure",
        BuildCommandErrorKind::ActionClosure(_) => "action_closure",
    }
}

fn native_kind(error: &NativeDemandSessionError) -> &'static str {
    match error {
        NativeDemandSessionError::Busy => "busy",
        NativeDemandSessionError::LeaseTokenExhausted => "lease_token_exhausted",
        NativeDemandSessionError::StaleLease => "stale_lease",
        NativeDemandSessionError::Repository(_) => "repository",
        NativeDemandSessionError::SourceService(_) => "source_service",
        NativeDemandSessionError::ConflictingRepository(_) => "conflicting_repository",
        NativeDemandSessionError::RepositoryInternalNonProgress => "repository_nonprogress",
        NativeDemandSessionError::EnvironmentInternalNonProgress => "environment_nonprogress",
        NativeDemandSessionError::DownloadInternalNonProgress => "download_nonprogress",
        NativeDemandSessionError::DownloadRequestConflict(_) => "download_conflict",
        NativeDemandSessionError::ForeignDownloadWorkspace { .. } => "foreign_download_workspace",
        NativeDemandSessionError::DownloadCapture(_) => "download_capture",
        NativeDemandSessionError::InputImage(_) => "input_image",
        NativeDemandSessionError::MissingSelectedDownload(_) => "missing_selected_download",
        NativeDemandSessionError::ForeignEnvironmentWorkspace { .. } => {
            "foreign_environment_workspace"
        }
        NativeDemandSessionError::PathInternalNonProgress => "path_nonprogress",
        NativeDemandSessionError::ExecuteInternalNonProgress => "execute_nonprogress",
        NativeDemandSessionError::ExecuteEnvironmentMismatch => "execute_environment_mismatch",
        NativeDemandSessionError::RepositoryExecute(_) => "repository_execute",
        NativeDemandSessionError::ExecuteTranscript(_) => "execute_transcript",
        NativeDemandSessionError::RevisionInternalNonProgress => "revision_nonprogress",
        NativeDemandSessionError::Revision(_) => "revision",
        NativeDemandSessionError::AfterOutputPublication(_) => "after_output_publication",
        NativeDemandSessionError::OutputPublicationAbort { .. } => "output_publication_abort",
        NativeDemandSessionError::MissingSelectedPath(_) => "missing_selected_path",
        NativeDemandSessionError::PathEpoch(_) => "path_epoch",
        NativeDemandSessionError::ObservedTerminal(mismatch) => match mismatch {
            ObservedTerminalMismatch::RepositoryRequests => "observed_terminal_repository_requests",
            ObservedTerminalMismatch::RepositoryValidations => {
                "observed_terminal_repository_validations"
            }
            ObservedTerminalMismatch::Length => "observed_terminal_length",
            ObservedTerminalMismatch::Demand => "observed_terminal_demand",
            ObservedTerminalMismatch::Value => "observed_terminal_value",
            ObservedTerminalMismatch::ResultArc => "observed_terminal_result_arc",
        },
        NativeDemandSessionError::Effect(_) => "effect",
        NativeDemandSessionError::ForeignEffects => "foreign_effects",
        NativeDemandSessionError::Computation(_) => "computation",
        NativeDemandSessionError::Injection(_) => "injection",
        NativeDemandSessionError::Restoration(_) => "restoration",
    }
}

#[test]
fn terminal_mismatch_subtypes_preserve_original_scope_refusal() {
    use crate::runtime::dice::source_service::SourceServiceError;

    let deadline = Instant::now() + Duration::from_secs(2);
    let scope = SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/terminal-mismatch-diagnostic").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    assert_eq!(
        scope.poison(SourceServiceError::NetworkDisabled),
        SourceServiceError::NetworkDisabled
    );
    for (mismatch, expected) in [
        (
            ObservedTerminalMismatch::RepositoryRequests,
            "observed_terminal_repository_requests",
        ),
        (
            ObservedTerminalMismatch::RepositoryValidations,
            "observed_terminal_repository_validations",
        ),
        (ObservedTerminalMismatch::Length, "observed_terminal_length"),
        (ObservedTerminalMismatch::Demand, "observed_terminal_demand"),
        (ObservedTerminalMismatch::Value, "observed_terminal_value"),
        (
            ObservedTerminalMismatch::ResultArc,
            "observed_terminal_result_arc",
        ),
    ] {
        assert_eq!(
            native_kind(&NativeDemandSessionError::ObservedTerminal(mismatch)),
            expected
        );
        assert_eq!(
            scope.first_error_for_test(),
            Some(SourceServiceError::NetworkDisabled)
        );
    }
    assert_eq!(
        native_kind(&NativeDemandSessionError::SourceService(
            SourceServiceError::NetworkDisabled
        )),
        "source_service"
    );
    assert_eq!(
        scope.poison(SourceServiceError::Cancelled),
        SourceServiceError::NetworkDisabled
    );
    assert_eq!(scope.check(), Err(SourceServiceError::NetworkDisabled));
}

fn resources(
    stage: &str,
    budgets: Option<&(
        crate::runtime::input_image::WorkspaceImageResources,
        slug_bzlmod_v2::SelectedScratchResources,
    )>,
) {
    if let Some((image, scratch)) = budgets {
        println!(
            "WP938_RESOURCES {stage} {} {} {} {}",
            image.descriptor_budget().used_bytes(),
            image.descriptor_budget().peak_reserved_bytes(),
            image.logical_bytes_used(),
            scratch.payload_budget().used_bytes(),
        );
    } else {
        println!("WP938_RESOURCES {stage} unavailable unavailable unavailable unavailable");
    }
}

#[test]
#[ignore = "one frozen offline normal command, launched only by the bounded supervisor"]
fn normal_rules_rust_public_probe_offline_source_prerequisite() {
    phase("baseline");
    let workspace = NormalizedAbsolutePath::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace"),
    )
    .expect("fixed fixture workspace is absolute");
    let root = std::env::var("SLUG_NATIVE_RULES_RUST_ROOT")
        .expect("supervisor supplies the frozen rules_rust root");
    let normalized_root = NormalizedAbsolutePath::new(Path::new(&root))
        .expect("supervisor supplies an absolute rules_rust root");
    assert_eq!(normalized_root.as_path(), Path::new(&root));
    assert!(root.len() <= 1024 && !root.chars().any(char::is_whitespace));
    let start = Instant::now();
    let scope = SourceServiceScope::new_for_test(
        workspace.clone(),
        start + Duration::from_secs(5),
        start + Duration::from_secs(8),
        SourceServiceLimits {
            allow_network: false,
            registry_object_bytes: 1 << 20,
            registry_attempts: 16,
            registry_admitted_bytes: 16 << 20,
            source_attempts: 16,
            transferred_bytes: 16 << 20,
            decoded_bytes: 16 << 20,
            written_bytes: 16 << 20,
            live_buffer_bytes: 16 << 20,
            blocking_threads: Some(2),
        },
    )
    .expect("fixed positive limits and deadlines are valid");
    let weak = Arc::downgrade(&scope);
    let (mut runtime, constructor_error) = match WorkspaceRuntime::new_with_source_services(
        workspace.as_path(),
        ProcessHostOwner::native(),
        WorkspaceSourceServices::Diagnostic(scope.clone()),
    ) {
        Ok(runtime) => (Some(runtime), None),
        Err(error) => (None, Some(error)),
    };
    let budgets = runtime.as_mut().map(|runtime| {
        let descriptor = slug_workspace_v2::ImageDescriptorBudget::new(16 << 20);
        runtime.image_resources =
            crate::runtime::input_image::WorkspaceImageResources::new(descriptor.clone(), 16 << 20);
        runtime.scratch_resources = slug_bzlmod_v2::SelectedScratchResources::new(
            slug_bzlmod_v2::SelectedScratchPayloadBudget::new(16 << 20),
            descriptor,
        );
        assert!(
            runtime
                .image_resources
                .descriptor_budget()
                .same_instance(runtime.scratch_resources.descriptor_budget(),)
        );
        (
            runtime.image_resources.clone(),
            runtime.scratch_resources.clone(),
        )
    });
    phase("after_runtime");
    resources("before_command", budgets.as_ref());
    println!(
        "WP938_REQUEST workspace={} rules_rust_root={} target=//pkg:probe setting=@rules_rust//rust/toolchain/channel value=nightly registry=https://bcr.bazel.build/ lockfile=off",
        workspace.as_path().display(),
        normalized_root.as_path().display()
    );

    let mut preparation_error = None;
    let mut policy_error = false;
    let mut outcome = None;
    if let Some(runtime) = &runtime {
        let module_override = format!("rules_rust={root}");
        match BzlmodCommandPolicyKey::from_flags_with_module_overrides(
            None,
            false,
            workspace.as_path(),
            [module_override.as_str()],
        ) {
            Err(_) => policy_error = true,
            Ok(policy) => {
                let environment =
                    BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
                        .expect("absent yanked-version environment value is valid");
                let targets =
                    [TargetPattern::parse("//pkg:probe").expect("fixed singleton pattern")];
                let overlay = CommandConfigurationOverlay::from(vec![
                    CommandConfigurationOccurrence::starlark(
                        "@rules_rust//rust/toolchain/channel",
                        Some("nightly"),
                        false,
                    ),
                ]);
                match runtime.prepare_build_request(
                    &targets,
                    policy,
                    environment,
                    LockfileMode::Off,
                    &["https://bcr.bazel.build/".to_owned()],
                    slug_bzlmod_v2::RepositoryEnvironmentSnapshot::default(),
                    overlay,
                ) {
                    Err(error) => preparation_error = Some(error),
                    Ok((key, request)) => {
                        assert!(BuildCommandRootObservationKey::new(key.clone()).is_none());
                        let singleton = SingletonRootSingleBuildCommandKey::new(key)
                            .expect("ordinary singleton public build dispatch");
                        outcome = Some(runtime.drive_command(request, singleton));
                    }
                }
            }
        }
    }
    phase("after_command");
    let (stage, kind) = if let Some(error) = &constructor_error {
        (
            "constructor_refusal",
            if error
                .downcast_ref::<crate::runtime::dice::source_service::SourceServiceError>()
                .is_some()
            {
                "source_service"
            } else if error.downcast_ref::<std::io::Error>().is_some() {
                "io"
            } else {
                "anyhow"
            },
        )
    } else if policy_error {
        ("policy_refusal", "module_overrides")
    } else if let Some(error) = &preparation_error {
        ("prepare_refusal", build_kind(error))
    } else {
        match outcome
            .as_ref()
            .expect("constructed and prepared command was driven once")
        {
            Err(error) => ("native_refusal", native_kind(error)),
            Ok(driven) => match driven.accepted.terminal().result.as_ref() {
                Err(error) => ("build_refusal", build_kind(error)),
                Ok(_) => ("build_complete", "complete"),
            },
        }
    };
    if let Some(Ok(driven)) = &outcome {
        let analyses = driven
            .accepted
            .terminal()
            .result
            .as_ref()
            .as_ref()
            .ok()
            .map(|evaluation| evaluation.analyses().count());
        println!(
            "WP938_OUTCOME {stage} {kind} {} {} {}",
            driven.attempts,
            driven.terminal_root_count,
            analyses.map_or_else(|| "unavailable".to_owned(), |n| n.to_string())
        );
    } else {
        println!("WP938_OUTCOME {stage} {kind} unavailable unavailable unavailable");
    }
    let counters = scope.snapshot();
    println!(
        "WP938_SCOPE {:?} {} {} {} {} {} {} {} {}",
        scope.first_error_for_test(),
        counters.registry_attempts,
        counters.registry_admitted_bytes,
        counters.source_attempts,
        counters.transferred_bytes,
        counters.decoded_bytes,
        counters.written_bytes,
        counters.live_buffer_bytes,
        counters.peak_buffer_bytes
    );
    resources("after_command", budgets.as_ref());
    drop(outcome);
    drop(preparation_error);
    drop(constructor_error);
    phase("after_terminal_drop");
    drop(runtime);
    phase("after_runtime_drop");
    resources("after_runtime_drop", budgets.as_ref());
    // Both final accounting and scope retirement share one bounded grace period.
    let retirement_deadline = Instant::now() + Duration::from_secs(2);
    while scope.snapshot().live_buffer_bytes != 0 && Instant::now() < retirement_deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    drop(scope);
    // DICE's StateProcessor can retire its final version graph asynchronously.
    while weak.upgrade().is_some() && Instant::now() < retirement_deadline {
        thread::sleep(Duration::from_millis(1));
    }
    assert!(
        weak.upgrade().is_none(),
        "scoped source owner survived final handle release"
    );
    phase("after_scope_drop");
    println!("WP938_RETIREMENT live_buffer_zero=true scope_weak_expired=true");
}
