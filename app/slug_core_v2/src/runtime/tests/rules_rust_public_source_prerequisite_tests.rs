use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::dice::source_service::RegistryAssociationSnapshot;
use crate::runtime::dice::source_service::RegistryRequestAssociation;
use crate::runtime::dice::source_service::SourceServiceCounters;
use crate::runtime::dice::source_service::SourceServiceError;
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
        NativeDemandSessionError::LegacyRepositoryExecuteUnsupported => {
            "legacy_execute_unsupported"
        }
        NativeDemandSessionError::ExecuteEnvironmentMismatch => "execute_environment_mismatch",
        #[cfg(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_pointer_width = "64"
        ))]
        NativeDemandSessionError::RepositoryWorkspace(_) => "repository_workspace",
        NativeDemandSessionError::ArchiveInternalNonProgress => "archive_internal_non_progress",
        NativeDemandSessionError::RepositoryFilesystem(_) => "repository_archive",
        NativeDemandSessionError::ArchiveTranscript(_) => "filesystem_transcript",
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

const REGISTRY_IDENTITY_BYTES: usize = 8192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FirstRowCapture {
    Missing,
    Captured,
    BoundsRefused,
}
impl FirstRowCapture {
    fn code(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Captured => "captured",
            Self::BoundsRefused => "bounds_refused",
        }
    }
}
struct FirstRegistryRow {
    bytes: [u8; REGISTRY_IDENTITY_BYTES * 2],
    state: FirstRowCapture,
    row_ordinal: u64,
    admissions: u64,
    url_len: usize,
    local_len: Option<usize>,
}
impl FirstRegistryRow {
    fn new() -> Self {
        Self {
            bytes: [0; REGISTRY_IDENTITY_BYTES * 2],
            state: FirstRowCapture::Missing,
            row_ordinal: 0,
            admissions: 0,
            url_len: 0,
            local_len: None,
        }
    }
    fn capture(
        &mut self,
        ordinal: u64,
        admissions: u64,
        raw_url: &[u8],
        local_path: Option<&[u8]>,
    ) {
        // Validate both domains before copying anything; refusal never truncates a row.
        if raw_url.len() > REGISTRY_IDENTITY_BYTES
            || local_path.is_some_and(|path| path.len() > REGISTRY_IDENTITY_BYTES)
        {
            self.state = FirstRowCapture::BoundsRefused;
            return;
        }
        self.bytes[..raw_url.len()].copy_from_slice(raw_url);
        if let Some(path) = local_path {
            self.bytes[REGISTRY_IDENTITY_BYTES..REGISTRY_IDENTITY_BYTES + path.len()]
                .copy_from_slice(path);
        }
        self.row_ordinal = ordinal;
        self.admissions = admissions;
        self.url_len = raw_url.len();
        self.local_len = local_path.map(<[u8]>::len);
        self.state = FirstRowCapture::Captured;
    }
    fn raw_url(&self) -> Option<&[u8]> {
        (self.state == FirstRowCapture::Captured).then(|| &self.bytes[..self.url_len])
    }
    fn local_path(&self) -> Option<&[u8]> {
        (self.state == FirstRowCapture::Captured)
            .then_some(self.local_len)
            .flatten()
            .map(|length| &self.bytes[REGISTRY_IDENTITY_BYTES..REGISTRY_IDENTITY_BYTES + length])
    }
}

// Borrowed streaming format: None and Some(empty) have distinct protocol spellings.
struct RegistryHex<'a>(Option<&'a [u8]>);
impl fmt::Display for RegistryHex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(bytes) = self.0 else {
            return formatter.write_str("none");
        };
        if bytes.is_empty() {
            return formatter.write_str("-");
        }
        for byte in bytes {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn sole_offline_registry_request(
    snapshot: &RegistryAssociationSnapshot,
    first: &FirstRegistryRow,
    counters: &SourceServiceCounters,
    error: Option<SourceServiceError>,
) -> bool {
    snapshot.complete
        && (
            snapshot.attempted_entries,
            snapshot.admitted_entries,
            snapshot.scope_admitted_entries,
            snapshot.admitted_distinct,
            snapshot.retained_rows,
        ) == (1, 1, 1, 1, 1)
        && (
            snapshot.admitted_repeated,
            snapshot.refused_entries,
            snapshot.sticky_refused_entries,
        ) == (0, 0, 0)
        && first.state == FirstRowCapture::Captured
        && first.row_ordinal == 1
        && first.admissions == 1
        && first.local_len.is_none()
        && first
            .raw_url()
            .is_some_and(|url| url.starts_with(b"https://bcr.bazel.build/"))
        && error == Some(SourceServiceError::NetworkDisabled)
        && counters.registry_attempts == 1
        && (
            counters.registry_admitted_bytes,
            counters.source_attempts,
            counters.transferred_bytes,
            counters.decoded_bytes,
            counters.written_bytes,
        ) == (0, 0, 0, 0, 0)
}

fn report_registry_association(
    recorder: &RegistryRequestAssociation,
    counters: &SourceServiceCounters,
    error: Option<SourceServiceError>,
) {
    let mut first = FirstRegistryRow::new();
    let mut visited = 0;
    let snapshot = recorder.visit_retained_rows(|row| {
        if visited == 0 {
            first.capture(row.row_ordinal, row.admissions, row.raw_url, row.local_path);
        }
        visited += 1; // Other rows have only scalar accounting, no second byte storage.
    });
    // All formatting/IO occurs after the synchronous visitor has released its lock.
    println!(
        "WP940_ASSOCIATION complete={} attempted={} admitted={} scope_admitted={} distinct={} repeated={} retained={} refused={} sticky={} charged={} visited={} capture={} qualified={}",
        snapshot.complete,
        snapshot.attempted_entries,
        snapshot.admitted_entries,
        snapshot.scope_admitted_entries,
        snapshot.admitted_distinct,
        snapshot.admitted_repeated,
        snapshot.retained_rows,
        snapshot.refused_entries,
        snapshot.sticky_refused_entries,
        snapshot.charged_bytes,
        visited,
        first.state.code(),
        sole_offline_registry_request(&snapshot, &first, counters, error)
    );
    if first.state == FirstRowCapture::Captured {
        // Avoid an encoded String or even a numeric path-length String in the native reporter.
        if let Some(path_len) = first.local_len {
            println!(
                "WP940_FIRST_ROW ordinal={} admissions={} url_len={} path_len={} url_hex={} path_hex={}",
                first.row_ordinal,
                first.admissions,
                first.url_len,
                path_len,
                RegistryHex(first.raw_url()),
                RegistryHex(first.local_path())
            );
        } else {
            println!(
                "WP940_FIRST_ROW ordinal={} admissions={} url_len={} path_len=none url_hex={} path_hex=none",
                first.row_ordinal,
                first.admissions,
                first.url_len,
                RegistryHex(first.raw_url())
            );
        }
    } else {
        println!("WP940_FIRST_ROW unavailable");
    }
}

#[test]
fn registry_first_row_reporter_preserves_raw_domains_bounds_and_offline_qualification() {
    let mut first = FirstRegistryRow::new();
    assert_eq!(first.state, FirstRowCapture::Missing);
    assert_eq!(first.raw_url(), None);
    first.capture(1, 1, b"\0u\xff", Some(b"\xff\0"));
    assert_eq!(first.raw_url(), Some(&b"\0u\xff"[..]));
    assert_eq!(first.local_path(), Some(&b"\xff\0"[..]));
    assert_eq!(RegistryHex(first.raw_url()).to_string(), "0075ff");
    assert_eq!(RegistryHex(first.local_path()).to_string(), "ff00");
    first.capture(1, 1, b"", Some(b""));
    assert_eq!(RegistryHex(first.raw_url()).to_string(), "-");
    assert_eq!(first.local_path(), Some(&b""[..]));
    assert_eq!(RegistryHex(first.local_path()).to_string(), "-");
    first.capture(1, 1, b"", None);
    assert_eq!(first.local_path(), None);
    assert_eq!(RegistryHex(first.local_path()).to_string(), "none");
    let bounded = [b'x'; REGISTRY_IDENTITY_BYTES + 1];
    first.capture(
        1,
        1,
        &bounded[..REGISTRY_IDENTITY_BYTES],
        Some(&bounded[..REGISTRY_IDENTITY_BYTES]),
    );
    assert_eq!(first.raw_url().unwrap().len(), REGISTRY_IDENTITY_BYTES);
    assert_eq!(first.local_path().unwrap().len(), REGISTRY_IDENTITY_BYTES);
    for (url, path) in [(&bounded[..], None), (&b"u"[..], Some(&bounded[..]))] {
        first.capture(1, 1, url, path);
        assert_eq!(first.state, FirstRowCapture::BoundsRefused);
        assert_eq!(first.raw_url(), None);
        assert_eq!(first.local_path(), None);
        assert!(first.bytes.iter().all(|&byte| byte == b'x'));
    }
    let snapshot = RegistryAssociationSnapshot {
        complete: true,
        attempted_entries: 1,
        admitted_entries: 1,
        scope_admitted_entries: 1,
        admitted_distinct: 1,
        retained_rows: 1,
        ..RegistryAssociationSnapshot::default()
    };
    // Retained recorder credit is live even when body/source acquisition bytes are zero.
    let counters = SourceServiceCounters {
        registry_attempts: 1,
        live_buffer_bytes: 1,
        peak_buffer_bytes: 1,
        ..SourceServiceCounters::default()
    };
    let offline = Some(SourceServiceError::NetworkDisabled);
    assert!(!sole_offline_registry_request(
        &snapshot, &first, &counters, offline
    ));
    first.state = FirstRowCapture::Missing;
    assert!(!sole_offline_registry_request(
        &snapshot, &first, &counters, offline
    ));
    first.capture(1, 1, b"https://bcr.bazel.build/exact", None);
    assert!(sole_offline_registry_request(
        &snapshot, &first, &counters, offline
    ));
    let changes: [fn(&mut RegistryAssociationSnapshot); 9] = [
        |s| s.complete = false,
        |s| s.attempted_entries = 2,
        |s| s.admitted_entries = 2,
        |s| s.scope_admitted_entries = 2,
        |s| s.admitted_distinct = 2,
        |s| s.retained_rows = 2,
        |s| s.admitted_repeated = 1,
        |s| s.refused_entries = 1,
        |s| s.sticky_refused_entries = 1,
    ];
    for change in changes {
        let mut rejected = snapshot;
        change(&mut rejected);
        assert!(!sole_offline_registry_request(
            &rejected, &first, &counters, offline
        ));
    }
    let byte_changes: [fn(&mut SourceServiceCounters); 6] = [
        |c| c.registry_attempts = 2,
        |c| c.registry_admitted_bytes = 1,
        |c| c.source_attempts = 1,
        |c| c.transferred_bytes = 1,
        |c| c.decoded_bytes = 1,
        |c| c.written_bytes = 1,
    ];
    for change in byte_changes {
        let mut rejected = counters;
        change(&mut rejected);
        assert!(!sole_offline_registry_request(
            &snapshot, &first, &rejected, offline
        ));
    }
    for (ordinal, admissions, url, path) in [
        (2, 1, &b"https://bcr.bazel.build/exact"[..], None),
        (1, 2, &b"https://bcr.bazel.build/exact"[..], None),
        (1, 1, &b"https://bcr.bazel.build/exact"[..], Some(&b""[..])),
        (
            1,
            1,
            &b"https://bcr.bazel.build/exact"[..],
            Some(&b"\xff"[..]),
        ),
        (1, 1, &b"https://bcr.bazel.build.evil/exact"[..], None),
        (1, 1, &b"http://bcr.bazel.build/exact"[..], None),
    ] {
        first.capture(ordinal, admissions, url, path);
        assert!(!sole_offline_registry_request(
            &snapshot, &first, &counters, offline
        ));
    }
    first.capture(1, 1, b"https://bcr.bazel.build/exact", None);
    for error in [
        None,
        Some(SourceServiceError::Cancelled),
        Some(SourceServiceError::WorkDeadline),
    ] {
        assert!(!sole_offline_registry_request(
            &snapshot, &first, &counters, error
        ));
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    let scope = SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/registry-reporter").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    scope.poison(SourceServiceError::NetworkDisabled);
    scope.cancel();
    assert!(sole_offline_registry_request(
        &snapshot,
        &first,
        &counters,
        scope.first_error_for_test()
    ));
    assert_eq!(scope.first_error_for_test(), offline);
}

#[test]
#[ignore = "one frozen offline normal command, launched only by the bounded supervisor"]
fn normal_rules_rust_public_probe_offline_source_prerequisite() {
    observe_normal_rules_rust_public_probe(false);
}

#[test]
#[ignore = "one frozen instrumented offline attribution, launched only by the bounded supervisor"]
fn normal_rules_rust_public_probe_offline_registry_attribution() {
    observe_normal_rules_rust_public_probe(true);
}

fn observe_normal_rules_rust_public_probe(arm_registry: bool) {
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
            live_decoder_bytes: SourceServiceLimits::MAX_LIVE_DECODER_BYTES,
            blocking_threads: Some(2),
        },
    )
    .expect("fixed positive limits and deadlines are valid");
    let weak = Arc::downgrade(&scope);
    let (recorder, arm_error) = if arm_registry {
        match scope.arm_registry_association_for_test() {
            Ok(recorder) => (Some(recorder), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let recorder_weak = recorder.as_ref().map(Arc::downgrade);
    let (mut runtime, constructor_error) = if arm_error.is_none() {
        match WorkspaceRuntime::new_with_source_services(
            workspace.as_path(),
            ProcessHostOwner::native(),
            WorkspaceSourceServices::Diagnostic(scope.clone()),
        ) {
            Ok(runtime) => (Some(runtime), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
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
    let (stage, kind) = if arm_error.is_some() {
        ("arm_refusal", "source_service")
    } else if let Some(error) = &constructor_error {
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
    if arm_registry {
        println!("WP940_ARM {arm_error:?}");
        if let Some(recorder) = &recorder {
            report_registry_association(recorder, &counters, scope.first_error_for_test());
        } else {
            println!("WP940_ASSOCIATION unavailable");
            println!("WP940_FIRST_ROW unavailable");
        }
    }
    resources("after_command", budgets.as_ref());
    drop(outcome);
    drop(preparation_error);
    drop(constructor_error);
    phase("after_terminal_drop");
    drop(runtime);
    phase("after_runtime_drop");
    resources("after_runtime_drop", budgets.as_ref());
    drop(recorder); // Release the deliberately retained charged slab before final zero/Weak checks.
    if let Some(recorder_weak) = recorder_weak {
        assert!(
            recorder_weak.upgrade().is_none(),
            "registry recorder survived final handle release"
        );
    }
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
    if arm_registry {
        println!(
            "WP938_RETIREMENT live_buffer_zero=true scope_weak_expired=true recorder_weak_expired=true"
        );
    } else {
        println!("WP938_RETIREMENT live_buffer_zero=true scope_weak_expired=true");
    }
}

#[cfg(target_os = "linux")]
mod platforms_module_preparation_tests {
    include!("platforms_module_preparation_tests.rs");
}
