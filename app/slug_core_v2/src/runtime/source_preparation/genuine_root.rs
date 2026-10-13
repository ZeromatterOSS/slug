//! Ignored, supervised actual-checkout source admission; never executes tools.
#[path = "refusal_receipt.rs"]
mod refusal_receipt;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::future::Future;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::pin::Pin;
use std::sync::Weak;

use sha2::Digest;
use sha2::Sha256;
use slug_bzlmod_v2::GuestLabelDataPolicy;
use slug_bzlmod_v2::GuestNamespaceLimits;
use slug_bzlmod_v2::GuestNamespacePolicy;
use slug_bzlmod_v2::RepositoryEnvironmentEntry;
use slug_bzlmod_v2::RepositoryEnvironmentSnapshot;
use slug_bzlmod_v2::RuntimeSourceDeclarationPolicy;
use slug_bzlmod_v2::RuntimeSourceLimits;
use slug_bzlmod_v2::SelectedScratchOperand;
use slug_bzlmod_v2::SelectedToolOperand;
use slug_bzlmod_v2::SelectedToolRole;
use slug_bzlmod_v2::SelectedToolSourcesPolicy;
use slug_workspace_v2::ElfMetadataOutcome;
use slug_workspace_v2::ElfMetadataPolicy;
use slug_workspace_v2::ImageChargedPath;
use slug_workspace_v2::ImageDescriptorBudget;
use slug_workspace_v2::ImageDescriptorReservation;

use super::harness_clock::HarnessClockStage;
use super::*;
use crate::runtime::dice::source_service::DiagnosticObserver;
use crate::runtime::dice::source_service::RegistryAssociationSnapshot;
use crate::runtime::dice::source_service::RegistryLimitRelation;
use crate::runtime::dice::source_service::SourceBufferKind;
use crate::runtime::dice::source_service::SourceBufferOrigin;
use crate::runtime::dice::source_service::SourceBufferSite;
use crate::runtime::dice::source_service::SourceBufferSnapshot;
use crate::runtime::dice::source_service::SourceBufferUnit;
use crate::runtime::dice::source_service::SourcePreparationDiagnosticRuntime;
use crate::runtime::dice::source_service::SourceServiceCounters;
use crate::runtime::dice::source_service::SourceServiceError;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::dice::source_service::SourceServiceScope;
use crate::runtime::dice::source_service::SourceTimeoutDomain;
use crate::runtime::dice::source_service::SourceTimeoutOrigin;
use crate::runtime::dice::source_service::SourceTimeoutPhase;
use crate::runtime::dice::source_service::SourceTimeoutReturn;
use crate::runtime::dice::source_service::SourceTimeoutSite;
use crate::runtime::dice::source_service::SourceTimeoutSnapshot;
use crate::runtime::input_image;

const WORKSPACE: &str = "/run/media/system/Colossus/dev/slug";
const RECEIPT_BYTES: usize = 64 * 1024;
const INPUTS: [(&str, u64, &str); 6] = [
    (
        "MODULE.bazel",
        2316,
        "109585b7b40d274ea912d32c73f54c25cc4bef1bc5e60526e18ac9bfe48d210c",
    ),
    (
        "MODULE.bazel.lock",
        58371,
        "bcdda78dd82aab5fd7ab1796985a78322b50178dceed7669c1b20173c2634302",
    ),
    (
        "Cargo.toml",
        12155,
        "9dc646f397f638644d5967a47fb4b7319bcbe8f6360824ff390b82518330495b",
    ),
    (
        "Cargo.lock",
        119614,
        "ee3ecfd672786ce09d9dc641280bf97ce55c86a67f615e20eb218007cd3ba314",
    ),
    (
        "Cargo.Bazel.lock",
        793783,
        "a0074e7ea15887124b22aefac681c2728dcef7430364fa9f7390eacac0e53d93",
    ),
    (
        ".bazelversion",
        6,
        "1b9487d55bea47fea50d226cc9c53bc548877ad2a318bac8fbc8b320f429e5c5",
    ),
];

// Owner before credit, including early receipt refusal.
struct Receipt {
    text: String,
    _credit: ImageDescriptorReservation,
}
impl Receipt {
    fn new(budget: &ImageDescriptorBudget) -> Result<Self, &'static str> {
        let credit = budget
            .try_reserve(RECEIPT_BYTES + 256)
            .map_err(|_| "receipt resource refusal")?;
        let mut text = String::new();
        text.try_reserve_exact(RECEIPT_BYTES)
            .map_err(|_| "receipt allocation refusal")?;
        if text.capacity() > RECEIPT_BYTES {
            return Err("receipt allocation capacity refusal");
        }
        Ok(Self {
            text,
            _credit: credit,
        })
    }
    fn number(&mut self, key: &'static str, value: u64) -> Result<(), &'static str> {
        if !key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
            || self
                .text
                .len()
                .checked_add(key.len() + 64)
                .is_none_or(|n| n > RECEIPT_BYTES)
        {
            return Err("receipt schema refusal");
        }
        writeln!(self.text, "SLUG_SOURCE_ADMISSION {key}={value}")
            .map_err(|_| "receipt schema refusal")
    }
    fn hex(&mut self, key: &'static str, bytes: &[u8]) -> Result<(), &'static str> {
        if bytes.len() > 4096
            || !key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
            || bytes
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(key.len() + 32))
                .and_then(|n| n.checked_add(self.text.len()))
                .is_none_or(|n| n > RECEIPT_BYTES)
        {
            return Err("receipt schema refusal");
        }
        write!(self.text, "SLUG_SOURCE_ADMISSION {key}=").map_err(|_| "receipt schema refusal")?;
        for byte in bytes {
            write!(self.text, "{byte:02x}").map_err(|_| "receipt schema refusal")?;
        }
        self.text.push('\n');
        Ok(())
    }
    fn counters(&mut self, c: SourceServiceCounters) -> Result<(), &'static str> {
        for (key, value) in [
            ("registry_attempts", c.registry_attempts),
            ("registry_bytes", c.registry_admitted_bytes),
            ("source_attempts", c.source_attempts),
            ("transferred_bytes", c.transferred_bytes),
            ("decoded_bytes", c.decoded_bytes),
            ("written_bytes", c.written_bytes),
            ("live_source_buffer_bytes", c.live_buffer_bytes as u64),
            ("source_buffer_peak_bytes", c.peak_buffer_bytes as u64),
        ] {
            self.number(key, value)?;
        }
        Ok(())
    }
}

fn pin_inputs(workspace: &Path, deadline: std::time::Instant) -> Result<(), &'static str> {
    let mut buffer = [0u8; 64 * 1024];
    for (relative, length, expected) in INPUTS {
        if std::time::Instant::now() >= deadline {
            return Err("input pin deadline refusal");
        }
        let path = workspace.join(relative);
        let named = std::fs::symlink_metadata(&path).map_err(|_| "input pin refusal")?;
        if !named.file_type().is_file() || named.len() != length {
            return Err("input pin refusal");
        }
        let mut file = File::open(&path).map_err(|_| "input pin refusal")?;
        let opened = file.metadata().map_err(|_| "input pin refusal")?;
        if named.dev() != opened.dev() || named.ino() != opened.ino() {
            return Err("input pin refusal");
        }
        let mut hash = Sha256::new();
        let mut count = 0u64;
        loop {
            if std::time::Instant::now() >= deadline {
                return Err("input pin deadline refusal");
            }
            let n = file.read(&mut buffer).map_err(|_| "input pin refusal")?;
            if n == 0 {
                break;
            }
            count = count.checked_add(n as u64).ok_or("input pin refusal")?;
            if count > length {
                return Err("input pin refusal");
            }
            hash.update(&buffer[..n]);
        }
        let digest: [u8; 32] = hash.finalize().into();
        let after = file.metadata().map_err(|_| "input pin refusal")?;
        let named_after = std::fs::symlink_metadata(&path).map_err(|_| "input pin refusal")?;
        if count != length
            || !digest_hex_matches(&digest, expected)
            || opened.dev() != after.dev()
            || opened.ino() != after.ino()
            || opened.len() != after.len()
            || opened.mtime_nsec() != after.mtime_nsec()
            || opened.mtime() != after.mtime()
            || opened.ctime() != after.ctime()
            || opened.ctime_nsec() != after.ctime_nsec()
            || after.dev() != named_after.dev()
            || after.ino() != named_after.ino()
        {
            return Err("input pin refusal");
        }
    }
    Ok(())
}
fn digest_hex_matches(bytes: &[u8; 32], expected: &str) -> bool {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    expected.len() == 64
        && bytes.iter().enumerate().all(|(i, b)| {
            expected.as_bytes()[2 * i] == HEX[(b >> 4) as usize]
                && expected.as_bytes()[2 * i + 1] == HEX[(b & 15) as usize]
        })
}

fn environment() -> Result<RepositoryEnvironmentSnapshot, &'static str> {
    let mut rows = BTreeMap::new();
    let mut bytes = 0usize;
    // No second ambient read, and no name/value appears in a refusal.
    for (name, value) in std::env::vars_os() {
        let name = name
            .into_string()
            .map_err(|_| "environment unicode refusal")?;
        let value = value
            .into_string()
            .map_err(|_| "environment unicode refusal")?;
        bytes = bytes
            .checked_add(name.len())
            .and_then(|n| n.checked_add(value.len()))
            .ok_or("environment bound refusal")?;
        if rows.len() >= 1024 || bytes > 256 * 1024 {
            return Err("environment bound refusal");
        }
        if rows.insert(name, Arc::<str>::from(value)).is_some() {
            return Err("environment canonical refusal");
        }
    }
    RepositoryEnvironmentSnapshot::from_canonical(Arc::from(
        rows.into_iter()
            .map(|(name, value)| RepositoryEnvironmentEntry::new(name, value))
            .collect::<Vec<_>>(),
    ))
    .map_err(|_| "environment canonical refusal")
}

fn query_shape<'a>(length: usize, mut operand: impl FnMut(usize) -> (u8, &'a str)) -> bool {
    if length != 12 {
        return false;
    }
    for (i, text) in [
        (1, "query"),
        (2, "--lockfile"),
        (4, "--config"),
        (6, "--splicing-manifest"),
        (8, "--cargo"),
        (10, "--rustc"),
    ] {
        if operand(i) != (0, text) {
            return false;
        }
    }
    // 0 is source-bound scratch or Label, data lock is Label, serialized inputs
    // are scratch, and both host tools are normal external Label operands.
    matches!(operand(0).0, 1 | 2)
        && operand(3).0 == 2
        && operand(5).0 == 1
        && operand(7).0 == 1
        && operand(9).0 == 2
        && operand(11).0 == 2
}
fn operand_kind(operand: &SelectedScratchOperand) -> u8 {
    match operand {
        SelectedScratchOperand::Literal(_) => 0,
        SelectedScratchOperand::Scratch(_) => 1,
        SelectedScratchOperand::ExternalPath(_) => 2,
    }
}

struct Observer {
    prior: AcceptedNativeDemandSnapshot,
    engine: Weak<Dice>,
    scope: Weak<SourceServiceScope>,
    images: input_image::WorkspaceImageResources,
    resources: slug_bzlmod_v2::SelectedScratchResources,
    restored: Cell<bool>,
    raw_restored: Cell<bool>,
    before_capture: Cell<bool>,
    after_capture: Cell<bool>,
    progress: Cell<u64>,
    progress_origins: Cell<super::progress_origins::ProgressOrigins>,
    refreshes: Cell<u64>,
    stage_contained: Cell<bool>,
    stage_paths_observed: Cell<u64>,
    source_samples_observed: Cell<bool>,
    descriptor_sample: Cell<usize>,
    counters: Cell<SourceServiceCounters>,
    poison: Cell<Option<SourceServiceError>>,
}
impl Observer {
    fn new(runtime: &WorkspaceRuntime, scope: &Arc<SourceServiceScope>) -> Self {
        Self {
            prior: runtime
                .native_demand_sessions
                .state
                .lock()
                .unwrap()
                .accepted
                .clone(),
            engine: Arc::downgrade(&runtime.dice),
            scope: Arc::downgrade(scope),
            images: runtime.image_resources.clone(),
            resources: runtime.scratch_resources.clone(),
            restored: Cell::new(false),
            raw_restored: Cell::new(false),
            before_capture: Cell::new(false),
            after_capture: Cell::new(false),
            progress: Cell::new(0),
            progress_origins: Cell::new(super::progress_origins::ProgressOrigins::default()),
            refreshes: Cell::new(0),
            stage_contained: Cell::new(true),
            stage_paths_observed: Cell::new(0),
            source_samples_observed: Cell::new(false),
            descriptor_sample: Cell::new(0),
            counters: Cell::new(SourceServiceCounters::default()),
            poison: Cell::new(None),
        }
    }
    fn hook(
        &self,
        phase: selected_tool_source::ToolPhase<'_>,
        stage: &Path,
    ) -> Result<(), NativeDemandSessionError> {
        self.descriptor_sample.set(
            self.descriptor_sample
                .get()
                .max(self.images.descriptor_budget().used_bytes()),
        );
        match phase {
            selected_tool_source::ToolPhase::ProgressFrom(origin) => {
                let mut counts = self.progress_origins.get();
                counts.record(origin);
                self.progress_origins.set(counts);
            }
            selected_tool_source::ToolPhase::BeforeCapture => self.before_capture.set(true),
            selected_tool_source::ToolPhase::AfterCapture => self.after_capture.set(true),
            selected_tool_source::ToolPhase::BeforeProgress(_) => {
                self.progress.set(self.progress.get() + 1)
            }
            selected_tool_source::ToolPhase::OwnerRefreshed(_, _) => {
                self.refreshes.set(self.refreshes.get() + 1)
            }
            selected_tool_source::ToolPhase::CaptureActivity(path) => {
                self.stage_paths_observed
                    .set(self.stage_paths_observed.get() + 1);
                if !path.starts_with(stage) {
                    self.stage_contained.set(false);
                    return Err(refusal("capture stage refusal"));
                }
            }
            selected_tool_source::ToolPhase::BeforeRestore(stages)
            | selected_tool_source::ToolPhase::AfterRestore(stages) => {
                let mut valid = true;
                stages.visit_stage_paths(&mut |path| {
                    self.stage_paths_observed
                        .set(self.stage_paths_observed.get() + 1);
                    valid &= path.starts_with(stage);
                });
                if !valid {
                    self.stage_contained.set(false);
                    return Err(refusal("capture stage refusal"));
                }
            }
        }
        Ok(())
    }
}
impl DiagnosticObserver for Observer {
    fn after_restore<'a>(
        &'a self,
        runtime: &'a WorkspaceRuntime,
        _: &'a Result<PreparedSelectedSourceView, NativeDemandSessionError>,
    ) -> Pin<Box<dyn Future<Output = Result<(), NativeDemandSessionError>> + 'a>> {
        Box::pin(async move {
            if let Some(scope) = runtime.source_scope() {
                self.counters.set(scope.snapshot());
                self.poison.set(scope.first_error_for_test());
                self.source_samples_observed.set(true);
            }
            {
                let state = runtime.native_demand_sessions.state.lock().unwrap();
                let now = &state.accepted;
                let before = &self.prior;
                let equal = now.inputs == before.inputs
                    && now.repository_results == before.repository_results
                    && now.path_observations == before.path_observations
                    && now.repository_environment_frontier
                        == before.repository_environment_frontier
                    && now.download_results == before.download_results
                    && now.selected == before.selected
                    && now.events == before.events
                    && matches!(state.phase, NativeDemandLeasePhase::Idle);
                self.restored.set(equal);
                if !equal {
                    return Err(refusal("snapshot restoration refusal"));
                }
            }
            // Only existing caller raw producers; no owner, source, progress or
            // injection. Root runs this future under the SAME cleanup deadline.
            let mut tx = runtime.dice.updater().commit().await;
            let workspace = runtime.workspace.clone();
            let prior = &self.prior.inputs.request;
            macro_rules! raw {
                ($key:expr, $expected:expr) => {
                    if tx
                        .compute(&$key)
                        .await
                        .map_err(|_| refusal("raw restoration refusal"))?
                        != $expected
                    {
                        return Err(refusal("raw restoration refusal"));
                    }
                };
            }
            raw!(
                RootModuleCommandPolicyKey {
                    workspace: workspace.clone()
                },
                slug_bzlmod_v2::RootModuleCommandPolicy::from(prior.command_policy.clone())
            );
            raw!(
                RootModuleEnvironmentPolicyKey {
                    workspace: workspace.clone()
                },
                slug_bzlmod_v2::RootModuleEnvironmentPolicy::from(prior.environment_policy.clone())
            );
            raw!(
                RootModuleLockfileModeKey {
                    workspace: workspace.clone()
                },
                slug_bzlmod_v2::RootModuleLockfileMode::from(prior.lockfile_mode.clone())
            );
            raw!(
                RootModuleRegistryUrlsKey {
                    workspace: workspace.clone()
                },
                slug_bzlmod_v2::RootModuleRegistryUrls::from(prior.registry_urls.clone())
            );
            let normalized = runtime.repository_materializer.workspace().clone();
            raw!(
                slug_bzlmod_v2::RepositoryEnvironmentSnapshotKey::new(normalized.clone()),
                prior.repository_environment.clone()
            );
            raw!(
                slug_bzlmod_v2::RuntimeSourceDeclarationPolicyKey::new(normalized.clone()),
                prior.runtime_sources.clone()
            );
            raw!(
                slug_bzlmod_v2::RawGuestNamespacePolicyKey::new(normalized),
                prior.guest_namespace
            );
            drop(tx);
            runtime.dice.wait_for_idle().await;
            self.raw_restored.set(true);
            Ok(())
        })
    }
}

// Default zero fields are not observations. A vacuous containment predicate
// cannot certify stage ownership before any actual retained path was visited.
fn observation_flags(sampled: bool, paths: u64, contained: bool) -> (u64, u64) {
    (u64::from(sampled), u64::from(paths > 0 && contained))
}

fn poison_code(error: Option<SourceServiceError>) -> u64 {
    match error {
        None => 0,
        Some(SourceServiceError::InvalidLimits) => 1,
        Some(SourceServiceError::ForeignScope) => 2,
        Some(SourceServiceError::AcquisitionClosed) => 3,
        Some(SourceServiceError::Cancelled) => 4,
        Some(SourceServiceError::WorkDeadline) => 5,
        Some(SourceServiceError::CleanupDeadline) => 6,
        Some(SourceServiceError::Limit(_)) => 7,
        Some(SourceServiceError::Overflow(_)) => 8,
        Some(SourceServiceError::BufferLimit) => 9,
        Some(SourceServiceError::Allocation) => 10,
        Some(SourceServiceError::UnsupportedSourceOwner) => 11,
        Some(SourceServiceError::NetworkDisabled) => 12,
        Some(SourceServiceError::DecoderLimit) => 13,
    }
}
fn outer_code(error: &NativeDemandSessionError) -> u64 {
    match error {
        NativeDemandSessionError::SourceService(_) => 1,
        NativeDemandSessionError::Repository(_) => 2,
        NativeDemandSessionError::InputImage(_) => 3,
        NativeDemandSessionError::Computation(_) => 4,
        NativeDemandSessionError::DownloadCapture(_) => 5,
        NativeDemandSessionError::Restoration(_) => 6,
        NativeDemandSessionError::Injection(_) => 7,
        _ => 8,
    }
}

fn emit_registry_association(
    snapshot: RegistryAssociationSnapshot,
    settled: bool,
    installed_scope: bool,
    first_poison: Option<SourceServiceError>,
    selected_native: Option<slug_bzlmod_v2::RegistryRefusalCause>,
    receipt: &mut Receipt,
) -> Result<(), &'static str> {
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeCounter;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;

    let complete = settled
        && installed_scope
        && snapshot.complete
        && snapshot.scope_admitted_entries == snapshot.admitted_entries
        && snapshot
            .admitted_distinct
            .checked_add(snapshot.admitted_repeated)
            == Some(snapshot.admitted_entries);
    let (relation, row, prior_count, prospective_unique) = match snapshot.first_limit_relation {
        RegistryLimitRelation::Unknown => (0, 0, 0, 0),
        RegistryLimitRelation::ExistingRow {
            row_ordinal,
            prior_admitted_count,
        } => (1, row_ordinal, prior_admitted_count, 0),
        RegistryLimitRelation::NewDistinctUnretained { unique_ordinal } => {
            (2, 0, 0, unique_ordinal)
        }
    };
    // Fresh runtime wiring and complete first-poison observation pair the causal
    // first exhaustion. They do not identify the selected leaf's own call.
    let paired = complete
        && relation != 0
        && snapshot.first_limit_call_ordinal.is_some()
        && first_poison
            == Some(SourceServiceError::Limit(
                source_service::SourceCounter::RegistryAttempts,
            ))
        && matches!(
            selected_native,
            Some(slug_bzlmod_v2::RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::Limit(
                    RegistryRefusalScopeCounter::RegistryAttempts
                )),
            })
        );
    for (key, value) in [
        ("registry_association_settled", u64::from(settled)),
        (
            "registry_association_installed_scope",
            u64::from(installed_scope),
        ),
        ("registry_identity_counts_complete", u64::from(complete)),
        (
            "registry_association_attempted_entries",
            snapshot.attempted_entries,
        ),
        (
            "registry_association_admitted_entries",
            snapshot.admitted_entries,
        ),
        (
            "registry_association_scope_admitted_entries",
            snapshot.scope_admitted_entries,
        ),
        (
            "registry_association_refused_entries",
            snapshot.refused_entries,
        ),
        (
            "registry_association_sticky_refusals",
            snapshot.sticky_refused_entries,
        ),
        (
            "registry_association_distinct_rows",
            snapshot.admitted_distinct,
        ),
        (
            "registry_association_repeated_entries",
            snapshot.admitted_repeated,
        ),
        ("registry_association_retained_rows", snapshot.retained_rows),
        (
            "registry_association_charged_bytes",
            snapshot.charged_bytes as u64,
        ),
        (
            "registry_first_limit_call_present",
            u64::from(snapshot.first_limit_call_ordinal.is_some()),
        ),
        (
            "registry_first_limit_call_ordinal",
            snapshot.first_limit_call_ordinal.unwrap_or(0),
        ),
        ("registry_first_limit_relation", relation),
        ("registry_first_limit_existing_row", row),
        ("registry_first_limit_prior_admissions", prior_count),
        (
            "registry_first_limit_prospective_unique",
            prospective_unique,
        ),
        ("registry_first_exhaustion_origin_paired", u64::from(paired)),
        ("registry_selected_leaf_call_identity_proved", 0),
    ] {
        receipt.number(key, value)?;
    }
    Ok(())
}

fn emit_timeout_provenance(
    snapshot: SourceTimeoutSnapshot,
    settled: bool,
    installed_scope: bool,
    first_poison: Option<SourceServiceError>,
    selected_native: Option<slug_bzlmod_v2::RegistryRefusalCause>,
    receipt: &mut Receipt,
) -> Result<(), &'static str> {
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;

    let declared_wait = snapshot.origin.is_some_and(|origin| {
        matches!(
            origin.site,
            SourceTimeoutSite::Dns
                | SourceTimeoutSite::Connect
                | SourceTimeoutSite::TlsHandshake
                | SourceTimeoutSite::HttpHandshake
                | SourceTimeoutSite::Headers
                | SourceTimeoutSite::Body
        ) && origin.domain != SourceTimeoutDomain::Uncomputed
            && matches!(
                origin.phase,
                SourceTimeoutPhase::DuringEntryCheck
                    | SourceTimeoutPhase::Timer
                    | SourceTimeoutPhase::Postcheck
                    | SourceTimeoutPhase::DuringFinalCheck
            )
            && origin.return_disposition == SourceTimeoutReturn::Deadline
            && (!origin.step_add_overflow || origin.domain == SourceTimeoutDomain::Common)
    });
    let proved = settled
        && installed_scope
        && snapshot.armed
        && snapshot.first_error_is_work_deadline
        && first_poison == Some(SourceServiceError::WorkDeadline)
        && declared_wait;
    let paired = proved
        && matches!(
            selected_native,
            Some(slug_bzlmod_v2::RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::WorkDeadline),
            })
        );
    // Raw fixed tags remain observations when proof is absent. Even pairing
    // certifies first scope origin, never the selected leaf's own caller/site.
    let origin = snapshot.origin;
    for (key, value) in [
        ("timeout_provenance_settled", u64::from(settled)),
        (
            "timeout_provenance_installed_scope",
            u64::from(installed_scope),
        ),
        ("timeout_provenance_armed", u64::from(snapshot.armed)),
        ("timeout_origin_present", u64::from(origin.is_some())),
        (
            "timeout_first_error_is_work_deadline",
            u64::from(snapshot.first_error_is_work_deadline),
        ),
        (
            "timeout_origin_site",
            origin.map_or(0, |o| u64::from(o.site.code())),
        ),
        (
            "timeout_origin_domain",
            origin.map_or(0, |o| u64::from(o.domain.code())),
        ),
        (
            "timeout_origin_phase",
            origin.map_or(0, |o| u64::from(o.phase.code())),
        ),
        (
            "timeout_origin_return",
            origin.map_or(0, |o| u64::from(o.return_disposition.code())),
        ),
        (
            "timeout_step_add_overflow",
            u64::from(origin.is_some_and(|o| o.step_add_overflow)),
        ),
        (
            "timeout_later_sticky_observed",
            u64::from(snapshot.later_sticky_work_deadline_observed),
        ),
        ("timeout_declared_wait_origin_proved", u64::from(proved)),
        ("timeout_first_scope_origin_paired", u64::from(paired)),
        ("timeout_selected_leaf_call_identity_proved", 0),
    ] {
        receipt.number(key, value)?;
    }
    Ok(())
}

fn emit_buffer_provenance(
    snapshot: SourceBufferSnapshot,
    settled: bool,
    installed_scope: bool,
    first_poison: Option<SourceServiceError>,
    selected_native: Option<slug_bzlmod_v2::RegistryRefusalCause>,
    receipt: &mut Receipt,
) -> Result<(), &'static str> {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;
    let origin = snapshot.origin;
    let proved = settled
        && installed_scope
        && snapshot.armed
        && snapshot.first_error_is_buffer_limit
        && first_poison == Some(SourceServiceError::BufferLimit)
        && origin.is_some_and(SourceBufferOrigin::complete);
    let paired = proved
        && matches!(
            selected_native,
            Some(RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::BufferLimit),
            })
        );
    for (key, value) in [
        ("buffer_provenance_settled", u64::from(settled)),
        (
            "buffer_provenance_installed_scope",
            u64::from(installed_scope),
        ),
        ("buffer_provenance_armed", u64::from(snapshot.armed)),
        (
            "buffer_first_error_is_buffer_limit",
            u64::from(snapshot.first_error_is_buffer_limit),
        ),
        ("buffer_origin_present", u64::from(origin.is_some())),
        ("buffer_origin_site", origin.map_or(0, |o| o.site.code())),
        ("buffer_origin_kind", origin.map_or(0, |o| o.kind.code())),
        ("buffer_origin_unit", origin.map_or(0, |o| o.unit.code())),
        (
            "buffer_origin_requested_present",
            u64::from(origin.is_some_and(|o| o.requested.is_some())),
        ),
        (
            "buffer_origin_requested",
            origin.and_then(|o| o.requested).map_or(0, |n| n as u64),
        ),
        (
            "buffer_origin_bound_present",
            u64::from(origin.is_some_and(|o| o.bound.is_some())),
        ),
        (
            "buffer_origin_bound",
            origin.and_then(|o| o.bound).map_or(0, |n| n as u64),
        ),
        (
            "buffer_origin_live_bytes",
            origin.map_or(0, |o| o.live_bytes as u64),
        ),
        (
            "buffer_origin_bank_ceiling_bytes",
            origin.map_or(0, |o| o.bank_ceiling_bytes as u64),
        ),
        (
            "buffer_reservation_add_overflow",
            u64::from(origin.is_some_and(|o| o.kind == SourceBufferKind::ReservationOverflow)),
        ),
        ("buffer_named_creator_proved", u64::from(proved)),
        ("buffer_first_scope_origin_paired", u64::from(paired)),
        ("buffer_selected_leaf_call_identity_proved", 0),
    ] {
        receipt.number(key, value)?;
    }
    Ok(())
}

fn emit_success(
    view: &source_service::ScopedPreparedSourceViewLease,
    inputs: &NativeDemandRequestInputBundle,
    receipt: &mut Receipt,
    stage: &Path,
) -> Result<(), &'static str> {
    if view.inputs() != inputs || !view.view().is_valid() {
        return Err("full caller proof refusal");
    }
    let facts = view.view().facts();
    let profile = facts.profile();
    let sources = profile.tool_sources();
    let observed = sources.prefix();
    let prefix = observed.prefix();
    let arguments = prefix.process().arguments();
    if !query_shape(arguments.len(), |i| {
        (operand_kind(&arguments[i]), arguments[i].spelling())
    }) || prefix.process().inherited_environment() != &inputs.repository_environment
        || prefix.owner() != view.selected().owner()
        || view.selected().owner().kind()
            != slug_bzlmod_v2::HostSelectedExtensionOwnerKind::ModuleExtension
    {
        return Err("actual query proof refusal");
    }
    let (definition, name, _, _) = observed.inputs().request().parts();
    if name != "crate"
        || definition.package().package().as_str() != "crate_universe"
        || definition.target().as_str() != "extensions.bzl"
    {
        return Err("actual owner proof refusal");
    }
    if profile.images().len() != 0
        || profile.requests().requests().len() != 0
        || view.view().sources().test_captured_count() != 0
        || view.view().sources().test_stage_count() != 0
    {
        return Err("empty runtime proof refusal");
    }
    receipt.hex(
        "canonical_repository_hex",
        view.selected().requested().as_str().as_bytes(),
    )?;
    receipt.hex(
        "owner_hex",
        view.selected().owner().unique_name().as_str().as_bytes(),
    )?;
    receipt.number("query_shape_proved", 1)?;
    receipt.number("full_caller_proved", 1)?;
    receipt.number(
        "owner_frontier_rows",
        view.selected().observations().observations().len() as u64,
    )?;
    receipt.number("scratch_operations", prefix.operations().len() as u64)?;
    receipt.number("scratch_final_entries", prefix.entries().len() as u64)?;
    receipt.number(
        "scratch_semantic_occurrence_bytes",
        prefix.occurrence_bytes() as u64,
    )?;
    receipt.number(
        "scratch_semantic_descriptor_bytes",
        prefix.descriptor_bytes() as u64,
    )?;
    receipt.number("guest_deduplicated_nodes", facts.nodes().len() as u64)?;
    receipt.number("extra_runtime_images", 0)?;
    let mut image_occurrences = 0u64;
    let mut image_bytes = 0u64;
    let mut image_identity = [None; 3];
    let tools = view.view().sources().tools();
    for (index, (source, expected)) in sources
        .sources()
        .zip(inputs_tool_policy()?.operands())
        .enumerate()
    {
        if source.operand() != *expected
            || !matches!(source.metadata().outcome(), ElfMetadataOutcome::Parsed(_))
        {
            return Err("tool role proof refusal");
        }
        let digest = source.metadata().digest();
        receipt.number("tool_role", index as u64)?;
        receipt.number(
            "tool_argument_index",
            source.operand().argument_index.into(),
        )?;
        receipt.hex("tool_sha256", digest.sha256())?;
        receipt.number("tool_size", digest.size_bytes())?;
        if let Some(result) = source.scratch_result() {
            let request = result.request();
            if request.expected_digest() != digest || request.prefix() != prefix {
                return Err("scratch proof refusal");
            }
            let download = request
                .blob()
                .download()
                .ok_or("actual generator download proof refusal")?;
            if download.request().sha256() != digest.sha256()
                || !observed
                    .download_requests()
                    .iter()
                    .any(|r| r == download.request())
            {
                return Err("actual generator download proof refusal");
            }
            receipt.number("tool_source_kind", 1)?;
            receipt.number("scratch_tool_entry", request.entry_index() as u64)?;
        } else {
            let selected = source.label_selection().ok_or("Label proof refusal")?;
            let image = source
                .label_metadata()
                .ok_or("Label ELF proof refusal")?
                .image();
            record_label_materialization(selected, image, stage, receipt)?;
            image_occurrences += image.entries().len() as u64;
            image_bytes = image_bytes
                .checked_add(image.total_file_bytes())
                .ok_or("image count refusal")?;
            image_identity[index] = Some(
                tools
                    .with_image_root(index, |file, actual| {
                        if actual != image {
                            return Err("capture binding refusal");
                        }
                        let stat = file.metadata().map_err(|_| "capture binding refusal")?;
                        Ok((stat.dev(), stat.ino()))
                    })
                    .ok_or("capture binding refusal")??,
            );
            receipt.number(
                "tool_materialization_namespace",
                u64::from(matches!(
                    image.namespace(),
                    slug_workspace_v2::PathObservationNamespace::Materialization(_)
                )),
            )?;
        }
    }
    record_capture_equivalence(sources, image_identity, receipt)?;
    receipt.number("tool_image_occurrence_entries", image_occurrences)?;
    receipt.number("tool_image_occurrence_bytes", image_bytes)?;
    Ok(())
}

// Bind honest producer storage, not merely our later output capture, to the
// actual normal immutable result and its exact current image namespace/anchor.
fn record_label_materialization(
    selected: &slug_bzlmod_v2::ObservedSelectedLabelSource,
    image: &slug_workspace_v2::ObservedInputImage,
    stage: &Path,
    receipt: &mut Receipt,
) -> Result<(), &'static str> {
    let (request, materialized) = selected
        .repository()
        .ok_or("materialization proof refusal")?;
    let slug_bzlmod_v2::RepositoryMaterialization::Immutable {
        canonical_repo,
        repo_spec,
        generation_root,
        observation_instance,
        ..
    } = materialized
    else {
        return Err("immutable producer proof refusal");
    };
    let root =
        NormalizedAbsolutePath::new(generation_root).map_err(|_| "producer root proof refusal")?;
    if !root.as_path().starts_with(stage)
        || root.as_path() == stage
        || image.anchor_root() != &root
        || image.namespace()
            != slug_workspace_v2::PathObservationNamespace::Materialization(*observation_instance)
        || canonical_repo != &request.id.canonical_repo
        || repo_spec != &request.repo_spec
    {
        return Err("producer binding proof refusal");
    }
    let slug_bzlmod_v2::RepositoryMaterializationKind::GeneratedFileEffects(plan) = &request.kind
    else {
        return Err("actual host tools proof refusal");
    };
    receipt.number("tool_source_kind", 2)?;
    for effect in plan.ordered_effects() {
        if let slug_bzlmod_v2::GeneratedRepositoryOrderedEffectRef::Archive(archive) = effect {
            receipt.hex("tool_archive_sha256", &archive.sha256())?;
        }
    }
    Ok(())
}

fn record_capture_equivalence(
    sources: &slug_loading_v2::ObservedSelectedExtensionToolSources,
    image_identity: [Option<(u64, u64)>; 3],
    receipt: &mut Receipt,
) -> Result<(), &'static str> {
    if sources.sources().len() != 3 || image_identity[1].is_none() || image_identity[2].is_none() {
        return Err("three role proof refusal");
    }
    let full_images_equal = sources
        .sources()
        .nth(1)
        .unwrap()
        .label_metadata()
        .unwrap()
        .image()
        == sources
            .sources()
            .nth(2)
            .unwrap()
            .label_metadata()
            .unwrap()
            .image();
    let cargo = image_identity[1].unwrap();
    let rustc = image_identity[2].unwrap();
    if !capture_equivalence(full_images_equal, cargo, rustc) {
        return Err("shared capture provenance refusal");
    }
    let shared = cargo == rustc;
    receipt.number(
        "producer_materializations_checked",
        image_identity.iter().flatten().count() as u64,
    )?;
    receipt.number("producer_containment_proved", 1)?;
    receipt.number("full_image_capture_equivalence_proved", 1)?;
    receipt.number("cargo_rustc_shared_capture", u64::from(shared))?;
    Ok(())
}

fn capture_equivalence(full_images_equal: bool, cargo: (u64, u64), rustc: (u64, u64)) -> bool {
    full_images_equal == (cargo == rustc)
}

fn inputs_tool_policy() -> Result<SelectedToolSourcesPolicy, &'static str> {
    SelectedToolSourcesPolicy::new(
        &[
            SelectedToolOperand {
                role: SelectedToolRole::Generator,
                argument_index: 0,
            },
            SelectedToolOperand {
                role: SelectedToolRole::Cargo,
                argument_index: 9,
            },
            SelectedToolOperand {
                role: SelectedToolRole::Rustc,
                argument_index: 11,
            },
        ],
        ElfMetadataPolicy::default(),
    )
    .map_err(|_| "tool policy refusal")
}

fn request_inputs(
    diagnostic: &SourcePreparationDiagnosticRuntime,
) -> Result<(NativeDemandRequestInputBundle, ImageChargedPath), &'static str> {
    let repository_environment = environment()?;
    let environment_policy = BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(
        repository_environment
            .get("BZLMOD_ALLOW_YANKED_VERSIONS")
            .map(|value| value.as_ref()),
    )
    .map_err(|_| "environment policy refusal")?;
    let pin = ImageChargedPath::pin_existing(
        diagnostic.workspace().clone(),
        diagnostic.descriptor_budget(),
    )
    .map_err(|_| "workspace policy resource refusal")?;
    let inputs = NativeDemandRequestInputBundle {
        archive_distdirs: crate::runtime::ArchiveDistdirPolicy::default(),
        runtime_sources: Some(
            RuntimeSourceDeclarationPolicy::new(
                &pin,
                &[],
                RuntimeSourceLimits::default(),
                diagnostic.descriptor_budget(),
            )
            .map_err(|_| "runtime policy refusal")?,
        ),
        guest_namespace: Some(
            GuestNamespacePolicy::new(
                1001,
                1002,
                1001,
                1002,
                9,
                7,
                8,
                0o755,
                GuestNamespaceLimits::default(),
            )
            .and_then(|p| {
                p.with_label_data(GuestLabelDataPolicy::IssuedFileLeaves { max_witnesses: 64 })
            })
            .map_err(|_| "guest policy refusal")?,
        ),
        command_policy: BzlmodCommandPolicyKey::from_flags(None, false)
            .map_err(|_| "command policy refusal")?,
        environment_policy,
        lockfile_mode: LockfileMode::Error,
        registry_urls: RegistryUrls::default_bazel_registry(),
        repository_environment,
    };
    Ok((inputs, pin))
}

fn run(
    receipt: &mut Receipt,
    clock: &HarnessClockStage,
    diagnostic: SourcePreparationDiagnosticRuntime,
    descriptor_settled: &Cell<bool>,
) -> Result<(), &'static str> {
    let observer = diagnostic.inspect_for_test(Observer::new);
    let baseline_descriptor = diagnostic.descriptor_budget().used_bytes();
    let baseline_payload = diagnostic.resources().payload_budget().used_bytes();
    let (inputs, pin) = request_inputs(&diagnostic)?;
    let name = ApparentRepoName::new("slug_crates").map_err(|_| "import refusal")?;
    let association = diagnostic
        .inspect_for_test(|runtime, scope| {
            if !runtime
                .source_scope()
                .is_some_and(|installed| Arc::ptr_eq(installed, scope))
            {
                return Err(SourceServiceError::ForeignScope);
            }
            let association = scope.arm_registry_association_for_test()?;
            if !association.same_scope(scope) {
                return Err(SourceServiceError::ForeignScope);
            }
            Ok(association)
        })
        .map_err(|_| "registry association setup refused")?;
    clock
        .revalidate_before_runtime()
        .map_err(|_| "setup revalidation refusal")?;
    let outcome = diagnostic.prepare_once_observed(
        SelectedSourcePreparationRequest {
            apparent_repository: &name,
            inputs: &inputs,
            scratch_policy: SelectedScratchPolicy::plain(),
            tool_policy: inputs_tool_policy()?,
        },
        &|phase| observer.hook(phase, clock.stage_path()),
        &observer,
    );
    // Raw restoration drains the last observation transaction before engine
    // retirement. All later retained reads below finish synchronously before
    // this function returns; only then may the outer receipt certify highwater.
    descriptor_settled.set(observer.raw_restored.get() && observer.engine.upgrade().is_none());
    receipt.number("progress_rounds", observer.progress.get())?;
    let origins = observer.progress_origins.get();
    for origin in selected_tool_source::ProgressOrigin::ALL {
        receipt.number(origin.receipt_key(), origins.count(origin))?;
    }
    receipt.number("progress_origins_valid", u64::from(origins.valid()))?;
    receipt.number("owner_refreshes", observer.refreshes.get())?;
    receipt.number("reached_capture", u64::from(observer.before_capture.get()))?;
    receipt.number("completed_capture", u64::from(observer.after_capture.get()))?;
    receipt.number(
        "stage_containment_proved",
        observation_flags(
            observer.source_samples_observed.get(),
            observer.stage_paths_observed.get(),
            observer.stage_contained.get(),
        )
        .1,
    )?;
    receipt.number("stage_paths_observed", observer.stage_paths_observed.get())?;
    receipt.number(
        "no_containment_violation_observed",
        u64::from(observer.stage_contained.get()),
    )?;
    receipt.number(
        "snapshot_restoration_proved",
        u64::from(observer.restored.get()),
    )?;
    receipt.number(
        "raw_restoration_proved",
        u64::from(observer.raw_restored.get()),
    )?;
    receipt.number(
        "engine_retired",
        u64::from(observer.engine.upgrade().is_none()),
    )?;
    receipt.number("first_poison_code", poison_code(observer.poison.get()))?;
    let source_sampled = observation_flags(
        observer.source_samples_observed.get(),
        observer.stage_paths_observed.get(),
        observer.stage_contained.get(),
    )
    .0;
    receipt.number("poison_observation_before_close", source_sampled)?;
    receipt.number("source_counters_sampled", source_sampled)?;
    receipt.counters(observer.counters.get())?;
    receipt.number(
        "descriptor_phase_sample_lower_bound",
        observer.descriptor_sample.get() as u64,
    )?;
    let selected_native = if let Err(error) = &outcome {
        receipt.number("outer_error_code", outer_code(error))?;
        refusal_receipt::emit(error, receipt)?
    } else {
        None
    };
    let settled = observer.restored.get()
        && observer.raw_restored.get()
        && observer.engine.upgrade().is_none();
    let first_poison = observer
        .scope
        .upgrade()
        .and_then(|scope| scope.first_error_for_test());
    let association_snapshot = association.snapshot();
    emit_registry_association(
        association_snapshot,
        settled,
        true,
        first_poison,
        selected_native,
        receipt,
    )?;
    emit_timeout_provenance(
        association_snapshot.timeout,
        settled,
        true,
        first_poison,
        selected_native,
        receipt,
    )?;
    emit_buffer_provenance(
        association_snapshot.buffer,
        settled,
        true,
        first_poison,
        selected_native,
        receipt,
    )?;
    // The recorder pins only this scope/bank. Retire it before the existing
    // exported-view/clone/last-Drop controls and on the refused path as well.
    drop(association);
    let view = outcome.map_err(|_| "actual source admission refused")?;
    if !observer.restored.get()
        || !observer.raw_restored.get()
        || observer.engine.upgrade().is_some()
        || !observer.stage_contained.get()
        || observer.stage_paths_observed.get() == 0
        || !observer.source_samples_observed.get()
    {
        return Err("restoration or retirement proof refusal");
    }
    emit_success(&view, &inputs, receipt, clock.stage_path())?;
    // A clone retains only the actual verified view; acquisition engine is gone.
    let clone = view.clone();
    drop(view);
    let node = clone.view().facts().nodes().iter().find(|node| matches!(node.binding(), slug_loading_v2::GuestNamespaceBinding::DataLeaf { .. })
        && matches!(node.kind(), slug_loading_v2::GuestNamespaceNodeKind::RegularFile(d) if d.size_bytes() <= 1 << 20))
        .ok_or("issued data read proof refusal")?;
    let slug_loading_v2::GuestNamespaceNodeKind::RegularFile(expected) = node.kind() else {
        return Err("issued data read proof refusal");
    };
    let mut hash = Sha256::new();
    let mut read_bytes = 0u64;
    clone
        .view()
        .stream_regular_for_test(
            node.path(),
            &mut |chunk| {
                hash.update(chunk);
                read_bytes += chunk.len() as u64;
            },
            &|| std::time::Instant::now() < clock.cleanup_deadline(),
        )
        .map_err(|_| "issued data read proof refusal")?;
    let digest: [u8; 32] = hash.finalize().into();
    if read_bytes != expected.size_bytes()
        || digest != *expected.sha256()
        || !clone.view().is_valid()
    {
        return Err("issued data read proof refusal");
    }
    receipt.hex("retained_data_read_sha256", &digest)?;
    receipt.number("retained_data_read_bytes", read_bytes)?;
    receipt.number("retained_clone_read_proved", 1)?;
    drop(clone);
    drop(inputs);
    drop(pin);
    if observer.images.logical_bytes_used() != 0
        || observer.resources.payload_budget().used_bytes() != baseline_payload
        || observer.images.descriptor_budget().used_bytes() != baseline_descriptor
        || observer.scope.upgrade().is_some()
    {
        return Err("last drop proof refusal");
    }
    receipt.number("last_drop_proved", 1)?;
    clock
        .revalidate_stage()
        .map_err(|_| "final stage identity refusal")?;
    pin_inputs(Path::new(WORKSPACE), clock.cleanup_deadline())?;
    receipt.number("input_pins_unchanged", 1)?;
    if std::time::Instant::now() >= clock.cleanup_deadline() {
        return Err("final cleanup deadline refusal");
    }
    Ok(())
}

#[test]
#[ignore = "ONE separately reviewed and supervised actual-root source-admission attempt"]
fn actual_slug_crates_source_admission() -> Result<(), &'static str> {
    let emitted = Cell::new(false);
    let outcome = actual_source_admission(&emitted);
    if !emitted.get() {
        println!(
            "SLUG_SOURCE_ADMISSION schema=1\nSLUG_SOURCE_ADMISSION status=0\nSLUG_SOURCE_ADMISSION setup_refusal=1"
        );
    }
    outcome
}

fn actual_source_admission(emitted: &Cell<bool>) -> Result<(), &'static str> {
    let clock =
        HarnessClockStage::from_environment().map_err(|_| "clock or stage setup refused")?;
    pin_inputs(Path::new(WORKSPACE), clock.work_deadline())?;
    clock
        .revalidate_before_runtime()
        .map_err(|_| "clock or stage setup refused")?;
    let workspace = NormalizedAbsolutePath::new(WORKSPACE).map_err(|_| "workspace refused")?;
    let limits = SourceServiceLimits::actual_root();
    let diagnostic = SourcePreparationDiagnosticRuntime::new(
        workspace,
        crate::runtime::ProcessHostOwner::native(),
        clock.work_deadline(),
        clock.cleanup_deadline(),
        limits,
    )
    .map_err(|_| "diagnostic constructor refused")?;
    let mut receipt = Receipt::new(diagnostic.descriptor_budget())?;
    receipt.number("schema", 1)?;
    receipt.number("registry_attempt_limit", limits.registry_attempts)?;
    receipt.number(
        "work_deadline_monotonic_ns",
        clock.work_deadline_monotonic_ns(),
    )?;
    receipt.number(
        "cleanup_deadline_monotonic_ns",
        clock.cleanup_deadline_monotonic_ns(),
    )?;
    receipt.number("full_runtime_closure_proved", 0)?;
    receipt.number("process_execution_proved", 0)?;
    receipt.number("historical_memory_attribution_proved", 0)?;
    let descriptor_budget = diagnostic.descriptor_budget().clone();
    let descriptor_settled = Cell::new(false);
    let outcome = run(&mut receipt, &clock, diagnostic, &descriptor_settled);
    // The run's locals and operation-local reader credits have now retired.
    // This lifetime maximum includes conservative receipt/policy/header and
    // conversion reservations; it is neither physical heap usage nor RSS.
    receipt.number(
        "descriptor_reserved_highwater_bytes",
        descriptor_budget.peak_reserved_bytes() as u64,
    )?;
    receipt.number(
        "descriptor_reserved_highwater_settled",
        u64::from(descriptor_settled.get()),
    )?;
    receipt.number(
        "descriptor_exact_peak_proved",
        u64::from(descriptor_settled.get()),
    )?;
    receipt.number("status", u64::from(outcome.is_ok()))?;
    print!("{}", receipt.text);
    emitted.set(true);
    outcome
}

#[test]
fn genuine_timeout_receipt_requires_settled_named_creator_and_exact_chosen_cause() {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;
    let origin = SourceTimeoutOrigin {
        site: SourceTimeoutSite::Headers,
        domain: SourceTimeoutDomain::Common,
        phase: SourceTimeoutPhase::Timer,
        return_disposition: SourceTimeoutReturn::Deadline,
        step_add_overflow: false,
        caller: None,
    };
    let snapshot = SourceTimeoutSnapshot {
        armed: true,
        first_error_is_work_deadline: true,
        origin: Some(origin),
        later_sticky_work_deadline_observed: true,
    };
    let poison = Some(SourceServiceError::WorkDeadline);
    let native = Some(RegistryRefusalCause {
        site: RegistryRefusalSite::Scope,
        detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::WorkDeadline),
    });
    let budget = ImageDescriptorBudget::new(128 * 1024);
    let mut receipt = Receipt::new(&budget).unwrap();
    emit_timeout_provenance(snapshot, true, true, poison, native, &mut receipt).unwrap();
    assert!(
        receipt
            .text
            .contains("timeout_declared_wait_origin_proved=1\n")
    );
    assert!(
        receipt
            .text
            .contains("timeout_first_scope_origin_paired=1\n")
    );
    assert!(
        receipt
            .text
            .contains("timeout_selected_leaf_call_identity_proved=0\n")
    );
    for expected in [
        "timeout_origin_site=7\n",
        "timeout_origin_domain=1\n",
        "timeout_origin_phase=3\n",
        "timeout_origin_return=1\n",
        "timeout_step_add_overflow=0\n",
        "timeout_later_sticky_observed=1\n",
    ] {
        assert!(receipt.text.contains(expected));
    }
    drop(receipt);
    assert_eq!(budget.used_bytes(), 0);
    for (site, domain, phase, overflow) in [
        (
            SourceTimeoutSite::Dns,
            SourceTimeoutDomain::Step,
            SourceTimeoutPhase::Timer,
            false,
        ),
        (
            SourceTimeoutSite::Connect,
            SourceTimeoutDomain::Tie,
            SourceTimeoutPhase::Postcheck,
            false,
        ),
        (
            SourceTimeoutSite::TlsHandshake,
            SourceTimeoutDomain::Common,
            SourceTimeoutPhase::DuringEntryCheck,
            false,
        ),
        (
            SourceTimeoutSite::HttpHandshake,
            SourceTimeoutDomain::Common,
            SourceTimeoutPhase::DuringFinalCheck,
            false,
        ),
        (
            SourceTimeoutSite::Headers,
            SourceTimeoutDomain::Common,
            SourceTimeoutPhase::Timer,
            false,
        ),
        (
            SourceTimeoutSite::Body,
            SourceTimeoutDomain::Common,
            SourceTimeoutPhase::Timer,
            true,
        ),
    ] {
        let sample = SourceTimeoutSnapshot {
            origin: Some(SourceTimeoutOrigin {
                site,
                domain,
                phase,
                step_add_overflow: overflow,
                ..origin
            }),
            ..snapshot
        };
        let mut receipt = Receipt::new(&budget).unwrap();
        emit_timeout_provenance(sample, true, true, poison, native, &mut receipt).unwrap();
        assert!(
            receipt
                .text
                .contains("timeout_declared_wait_origin_proved=1\n")
        );
        assert!(
            receipt
                .text
                .contains("timeout_first_scope_origin_paired=1\n")
        );
        assert!(
            receipt
                .text
                .contains("timeout_selected_leaf_call_identity_proved=0\n")
        );
        drop(receipt);
        assert_eq!(budget.used_bytes(), 0);
    }
    let wrong_native = Some(RegistryRefusalCause {
        site: RegistryRefusalSite::Scope,
        detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::Cancelled),
    });
    let wrong_site = native.map(|cause| RegistryRefusalCause {
        site: RegistryRefusalSite::Dns,
        ..cause
    });
    let variant = |changed| SourceTimeoutSnapshot {
        origin: Some(changed),
        ..snapshot
    };
    for (sample, settled, installed, first, selected) in [
        (snapshot, false, true, poison, native),
        (snapshot, true, false, poison, native),
        (
            SourceTimeoutSnapshot {
                armed: false,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
        ),
        (
            SourceTimeoutSnapshot {
                first_error_is_work_deadline: false,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
        ),
        (
            SourceTimeoutSnapshot {
                origin: None,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
        ),
        (snapshot, true, true, None, native),
        (
            snapshot,
            true,
            true,
            Some(SourceServiceError::Cancelled),
            native,
        ),
        (snapshot, true, true, poison, None),
        (snapshot, true, true, poison, wrong_native),
        (snapshot, true, true, poison, wrong_site),
        (
            variant(SourceTimeoutOrigin {
                return_disposition: SourceTimeoutReturn::CompletedPrimary,
                ..origin
            }),
            true,
            true,
            poison,
            native,
        ),
        (
            variant(SourceTimeoutOrigin {
                return_disposition: SourceTimeoutReturn::Unknown,
                ..origin
            }),
            true,
            true,
            poison,
            native,
        ),
        (
            variant(SourceTimeoutOrigin {
                site: SourceTimeoutSite::GenericScope,
                ..origin
            }),
            true,
            true,
            poison,
            native,
        ),
        (
            variant(SourceTimeoutOrigin {
                site: SourceTimeoutSite::GenericWait,
                ..origin
            }),
            true,
            true,
            poison,
            native,
        ),
        (
            variant(SourceTimeoutOrigin {
                domain: SourceTimeoutDomain::Uncomputed,
                ..origin
            }),
            true,
            true,
            poison,
            native,
        ),
        (
            variant(SourceTimeoutOrigin {
                phase: SourceTimeoutPhase::UnattributedPoison,
                ..origin
            }),
            true,
            true,
            poison,
            native,
        ),
        (
            variant(SourceTimeoutOrigin {
                phase: SourceTimeoutPhase::ScopeCheck,
                ..origin
            }),
            true,
            true,
            poison,
            native,
        ),
        (
            variant(SourceTimeoutOrigin {
                domain: SourceTimeoutDomain::Step,
                step_add_overflow: true,
                ..origin
            }),
            true,
            true,
            poison,
            native,
        ),
    ] {
        let mut receipt = Receipt::new(&budget).unwrap();
        emit_timeout_provenance(sample, settled, installed, first, selected, &mut receipt).unwrap();
        assert!(
            receipt
                .text
                .contains("timeout_first_scope_origin_paired=0\n")
        );
        assert!(
            receipt
                .text
                .contains("timeout_selected_leaf_call_identity_proved=0\n")
        );
        assert!(receipt.text.lines().all(|line| {
            line.strip_prefix("SLUG_SOURCE_ADMISSION ")
                .and_then(|r| r.split_once('='))
                .is_some_and(|(_, value)| value.parse::<u64>().is_ok())
        }));
        let origin_should_be_proved = settled
            && installed
            && sample.armed
            && sample.first_error_is_work_deadline
            && first == poison
            && sample.origin == Some(origin);
        assert!(receipt.text.contains(if origin_should_be_proved {
            "timeout_declared_wait_origin_proved=1\n"
        } else {
            "timeout_declared_wait_origin_proved=0\n"
        }));
        drop(receipt);
        assert_eq!(budget.used_bytes(), 0);
    }
}

#[test]
fn genuine_buffer_receipt_rejects_malformed_origins_and_keeps_native_pair_separate() {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;
    let limit = SourceServiceLimits::default().live_buffer_bytes;
    let origin = SourceBufferOrigin {
        site: SourceBufferSite::SelectedPayload,
        kind: SourceBufferKind::ReservationCeiling,
        unit: SourceBufferUnit::Bytes,
        requested: Some(2),
        bound: Some(limit),
        live_bytes: limit - 1,
        bank_ceiling_bytes: limit,
    };
    let snapshot = SourceBufferSnapshot {
        armed: true,
        first_error_is_buffer_limit: true,
        origin: Some(origin),
    };
    let poison = Some(SourceServiceError::BufferLimit);
    let native = Some(RegistryRefusalCause {
        site: RegistryRefusalSite::Scope,
        detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::BufferLimit),
    });
    let budget = ImageDescriptorBudget::new(128 * 1024);
    let mut receipt = Receipt::new(&budget).unwrap();
    emit_buffer_provenance(snapshot, true, true, poison, native, &mut receipt).unwrap();
    for expected in [
        "buffer_origin_site=8\n",
        "buffer_origin_kind=1\n",
        "buffer_origin_unit=1\n",
        "buffer_origin_requested_present=1\n",
        "buffer_origin_requested=2\n",
        "buffer_named_creator_proved=1\n",
        "buffer_first_scope_origin_paired=1\n",
        "buffer_selected_leaf_call_identity_proved=0\n",
    ] {
        assert!(receipt.text.contains(expected), "{expected}");
    }
    drop(receipt);
    assert_eq!(budget.used_bytes(), 0);
    let explicit = SourceBufferOrigin {
        site: SourceBufferSite::ArchiveUrlLength,
        kind: SourceBufferKind::ExplicitBound,
        requested: Some(8193),
        bound: Some(8192),
        ..origin
    };
    let dns = SourceBufferOrigin {
        site: SourceBufferSite::DnsCardinality,
        unit: SourceBufferUnit::Entries,
        requested: Some(65),
        bound: Some(64),
        ..explicit
    };
    let growth = SourceBufferOrigin {
        site: SourceBufferSite::SelectedPayloadGrowth,
        requested: Some(2),
        bound: Some(1),
        ..explicit
    };
    let overflow = SourceBufferOrigin {
        kind: SourceBufferKind::ReservationOverflow,
        requested: Some(usize::MAX),
        ..origin
    };
    let invalid = [
        SourceBufferOrigin {
            site: SourceBufferSite::Unattributed,
            ..origin
        },
        SourceBufferOrigin {
            site: SourceBufferSite::ArchiveUrlLength,
            ..origin
        },
        SourceBufferOrigin {
            unit: SourceBufferUnit::Entries,
            ..origin
        },
        SourceBufferOrigin {
            kind: SourceBufferKind::ExplicitBound,
            ..origin
        },
        SourceBufferOrigin {
            kind: SourceBufferKind::ReservationOverflow,
            ..origin
        },
        SourceBufferOrigin {
            requested: Some(1),
            ..origin
        },
        SourceBufferOrigin {
            requested: None,
            ..origin
        },
        SourceBufferOrigin {
            bound: None,
            ..origin
        },
        SourceBufferOrigin {
            bound: Some(limit - 1),
            ..origin
        },
        SourceBufferOrigin {
            live_bytes: limit + 1,
            ..origin
        },
        SourceBufferOrigin {
            bank_ceiling_bytes: 0,
            ..origin
        },
        SourceBufferOrigin {
            bank_ceiling_bytes: limit + 1,
            ..origin
        },
        SourceBufferOrigin {
            site: SourceBufferSite::SelectedPayload,
            ..explicit
        },
        SourceBufferOrigin {
            bound: Some(8191),
            ..explicit
        },
        SourceBufferOrigin {
            requested: Some(8192),
            ..explicit
        },
        SourceBufferOrigin {
            unit: SourceBufferUnit::Entries,
            ..explicit
        },
        SourceBufferOrigin {
            bound: Some(63),
            ..dns
        },
        SourceBufferOrigin {
            unit: SourceBufferUnit::Bytes,
            ..dns
        },
        SourceBufferOrigin {
            requested: Some(32 * 1024 * 1024 + 1),
            bound: Some(32 * 1024 * 1024),
            ..growth
        },
        SourceBufferOrigin {
            requested: Some(1),
            ..growth
        },
    ];
    for origin in invalid {
        let mut receipt = Receipt::new(&budget).unwrap();
        emit_buffer_provenance(
            SourceBufferSnapshot {
                origin: Some(origin),
                ..snapshot
            },
            true,
            true,
            poison,
            native,
            &mut receipt,
        )
        .unwrap();
        assert!(
            receipt.text.contains("buffer_named_creator_proved=0\n"),
            "{origin:?}"
        );
        assert!(
            receipt
                .text
                .contains("buffer_first_scope_origin_paired=0\n")
        );
        assert!(
            receipt
                .text
                .contains("buffer_selected_leaf_call_identity_proved=0\n")
        );
    }
    for origin in [explicit, dns, growth, overflow] {
        let mut receipt = Receipt::new(&budget).unwrap();
        emit_buffer_provenance(
            SourceBufferSnapshot {
                origin: Some(origin),
                ..snapshot
            },
            true,
            true,
            poison,
            None,
            &mut receipt,
        )
        .unwrap();
        assert!(receipt.text.contains("buffer_named_creator_proved=1\n"));
        assert!(
            receipt
                .text
                .contains("buffer_first_scope_origin_paired=0\n")
        );
    }
    for (sample, settled, installed, first, selected, proved, paired) in [
        (snapshot, false, true, poison, native, false, false),
        (snapshot, true, false, poison, native, false, false),
        (
            SourceBufferSnapshot {
                armed: false,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
            false,
            false,
        ),
        (
            SourceBufferSnapshot {
                first_error_is_buffer_limit: false,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
            false,
            false,
        ),
        (
            SourceBufferSnapshot {
                origin: None,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
            false,
            false,
        ),
        (snapshot, true, true, None, native, false, false),
        (
            snapshot,
            true,
            true,
            Some(SourceServiceError::Cancelled),
            native,
            false,
            false,
        ),
        (snapshot, true, true, poison, None, true, false),
        (
            snapshot,
            true,
            true,
            poison,
            Some(RegistryRefusalCause {
                site: RegistryRefusalSite::HttpBody,
                detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::BufferLimit),
            }),
            true,
            false,
        ),
        (
            snapshot,
            true,
            true,
            poison,
            Some(RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::WorkDeadline),
            }),
            true,
            false,
        ),
    ] {
        let mut receipt = Receipt::new(&budget).unwrap();
        emit_buffer_provenance(sample, settled, installed, first, selected, &mut receipt).unwrap();
        assert!(receipt.text.contains(&format!(
            "buffer_origin_present={}\n",
            u64::from(sample.origin.is_some())
        )));
        assert!(receipt.text.contains(&format!(
            "buffer_origin_requested_present={}\n",
            u64::from(sample.origin.is_some_and(|o| o.requested.is_some()))
        )));
        assert!(receipt.text.contains(&format!(
            "buffer_origin_requested={}\n",
            sample.origin.and_then(|o| o.requested).unwrap_or(0)
        )));
        assert!(receipt.text.contains(if proved {
            "buffer_named_creator_proved=1\n"
        } else {
            "buffer_named_creator_proved=0\n"
        }));
        assert!(receipt.text.contains(if paired {
            "buffer_first_scope_origin_paired=1\n"
        } else {
            "buffer_first_scope_origin_paired=0\n"
        }));
        assert!(
            receipt
                .text
                .contains("buffer_selected_leaf_call_identity_proved=0\n")
        );
    }
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn genuine_registry_association_receipt_requires_complete_settled_same_scope_origin() {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeCounter;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;
    let snapshot = RegistryAssociationSnapshot {
        complete: true,
        attempted_entries: 130,
        admitted_entries: 128,
        scope_admitted_entries: 128,
        admitted_distinct: 1,
        admitted_repeated: 127,
        refused_entries: 2,
        sticky_refused_entries: 1,
        retained_rows: 1,
        first_limit_call_ordinal: Some(129),
        first_limit_relation: RegistryLimitRelation::NewDistinctUnretained { unique_ordinal: 2 },
        charged_bytes: 2 << 20,
        timeout: SourceTimeoutSnapshot::default(),
        buffer: SourceBufferSnapshot::default(),
    };
    let native = Some(RegistryRefusalCause {
        site: RegistryRefusalSite::Scope,
        detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::Limit(
            RegistryRefusalScopeCounter::RegistryAttempts,
        )),
    });
    let poison = Some(SourceServiceError::Limit(
        source_service::SourceCounter::RegistryAttempts,
    ));
    let wrong_counter = Some(RegistryRefusalCause {
        site: RegistryRefusalSite::Scope,
        detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::Limit(
            RegistryRefusalScopeCounter::SourceAttempts,
        )),
    });
    let wrong_site = native.map(|cause| RegistryRefusalCause {
        site: RegistryRefusalSite::LocalRead,
        ..cause
    });
    let wrong_poison = Some(SourceServiceError::Limit(
        source_service::SourceCounter::SourceAttempts,
    ));
    let absent_call = RegistryAssociationSnapshot {
        first_limit_call_ordinal: None,
        ..snapshot
    };
    let unknown_relation = RegistryAssociationSnapshot {
        first_limit_relation: RegistryLimitRelation::Unknown,
        ..snapshot
    };
    let budget = ImageDescriptorBudget::new(128 * 1024);
    let mut receipt = Receipt::new(&budget).unwrap();
    emit_registry_association(snapshot, true, true, poison, native, &mut receipt).unwrap();
    assert!(
        receipt
            .text
            .contains("registry_first_exhaustion_origin_paired=1\n")
    );
    assert!(
        receipt
            .text
            .contains("registry_first_limit_call_ordinal=129\n")
    );
    assert!(
        receipt
            .text
            .contains("registry_first_limit_prospective_unique=2\n")
    );
    assert!(
        receipt
            .text
            .contains("registry_selected_leaf_call_identity_proved=0\n")
    );
    for (sample, settled, installed, first, selected) in [
        (snapshot, false, true, poison, native),
        (snapshot, true, false, poison, native),
        (
            RegistryAssociationSnapshot {
                complete: false,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
        ),
        (
            RegistryAssociationSnapshot {
                scope_admitted_entries: 129,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
        ),
        (
            RegistryAssociationSnapshot {
                admitted_repeated: 126,
                ..snapshot
            },
            true,
            true,
            poison,
            native,
        ),
        (snapshot, true, true, None, native),
        (snapshot, true, true, poison, None),
        (snapshot, true, true, poison, wrong_counter),
        (snapshot, true, true, poison, wrong_site),
        (snapshot, true, true, wrong_poison, native),
        (absent_call, true, true, poison, native),
        (unknown_relation, true, true, poison, native),
    ] {
        receipt.text.clear();
        emit_registry_association(sample, settled, installed, first, selected, &mut receipt)
            .unwrap();
        assert!(
            receipt
                .text
                .contains("registry_first_exhaustion_origin_paired=0\n")
        );
        assert!(
            receipt
                .text
                .lines()
                .all(|line| line.starts_with("SLUG_SOURCE_ADMISSION ")
                    && line.rsplit_once('=').unwrap().1.parse::<u64>().is_ok())
        );
    }
    drop(receipt);
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn genuine_receipt_schema_excludes_dynamic_errors_and_bounds_values() {
    let budget = ImageDescriptorBudget::new(128 * 1024);
    let mut receipt = Receipt::new(&budget).unwrap();
    receipt.number("outer_error_code", 4).unwrap();
    receipt
        .number(
            "first_poison_code",
            poison_code(Some(SourceServiceError::BufferLimit)),
        )
        .unwrap();
    assert!(receipt.number("credential=value", 1).is_err());
    assert!(receipt.hex("identity", &[0; 4097]).is_err());
    assert!(!receipt.text.contains("credential"));
    assert!(
        receipt
            .text
            .lines()
            .all(|line| line.starts_with("SLUG_SOURCE_ADMISSION "))
    );
    assert!(receipt.text.len() <= RECEIPT_BYTES);
    drop(receipt);
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn genuine_query_shape_requires_literal_flags_and_typed_source_slots() {
    let mut args = [
        (1, "generator"),
        (0, "query"),
        (0, "--lockfile"),
        (2, "lock"),
        (0, "--config"),
        (1, "config"),
        (0, "--splicing-manifest"),
        (1, "splicing"),
        (0, "--cargo"),
        (2, "cargo"),
        (0, "--rustc"),
        (2, "rustc"),
    ];
    assert!(query_shape(args.len(), |i| args[i]));
    args[9].0 = 0;
    assert!(!query_shape(args.len(), |i| args[i]));
    args[9].0 = 2;
    args[1].1 = "generate";
    assert!(!query_shape(args.len(), |i| args[i]));
    assert!(!query_shape(11, |i| args[i]));
}

#[test]
fn genuine_zero_observations_never_certify_default_samples_or_containment() {
    assert_eq!(observation_flags(false, 0, true), (0, 0));
    assert_eq!(observation_flags(true, 0, true), (1, 0));
    assert_eq!(observation_flags(true, 3, false), (1, 0));
    assert_eq!(observation_flags(true, 3, true), (1, 1));
}

#[test]
fn genuine_capture_sharing_requires_full_image_equality_in_both_directions() {
    let a = (11, 22);
    let b = (11, 23);
    assert!(capture_equivalence(true, a, a));
    assert!(capture_equivalence(false, a, b));
    assert!(!capture_equivalence(true, a, b));
    assert!(!capture_equivalence(false, a, a));
}
