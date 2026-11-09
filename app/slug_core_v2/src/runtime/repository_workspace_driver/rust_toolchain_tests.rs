//! One diagnostic over the genuine original Rust cquery root. Instrumentation
//! records bounded scalar metadata only and never supplies graph/native results.
use std::fmt::Write as _;
use std::io::Read as _;

use serde_json::Value;
use serde_json::json;
use sha2::Digest;
use slug_bzlmod_v2::RepositoryEnvironmentEntry;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_bzlmod_v2::RepositoryWorkspaceRequest;
use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_bzlmod_v2::SourcePreparationOutcome;
use slug_configuration_v2::CommandConfigurationOccurrence;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;

use super::*;

#[cfg(feature = "native-probe-observer")]
#[path = "rust_toolchain_tests/phase_profile.rs"]
mod phase_profile;

const EVENT_LIMIT: usize = 128;
const TRACE_BYTES: usize = 512 * 1024;
const PREFIX_EVENTS: usize = 32;
const CENSUS_BYTES: usize = 64 * 1024;
#[cfg(feature = "native-probe-observer")]
const PHASE_BYTES: usize = phase_profile::REPORT_BYTES;
#[cfg(not(feature = "native-probe-observer"))]
const PHASE_BYTES: usize = 0;
const EVENT_TRACE_BYTES: usize = TRACE_BYTES - CENSUS_BYTES - 4096 - PHASE_BYTES;
const CONTENT_IDENTITIES: usize = 65;
const CONTENT_PATH_BYTES: usize = CONTENT_IDENTITIES * 4096;
const EVENT_BYTES: usize = EVENT_TRACE_BYTES / (PREFIX_EVENTS + 1);
const CONFIG_BYTES: usize = 128 * 1024;
const REASON_BYTES: usize = 16 * 1024;
const ROW_LIMIT: usize = 8;
const OWNER_FILES: [&str; 6] = [
    "MODULE.bazel",
    "pkg/BUILD.bazel",
    "pkg/main.rs",
    "pkg/lib.rs",
    "pkg/data.txt",
    "cquery_format.bzl",
];

#[derive(Default)]
struct Trace {
    computes: usize,
    latest_epoch: Value,
    events: Vec<Value>,
    truncated: bool,
    encoded_bytes: usize,
    event_bytes: Vec<usize>,
    census: ContentCensus,
    pending_progress: Option<PendingProgress>,
    progress_totals: [ChargeTotals; 7],
    family_totals: [ChargeTotals; 7],
    invalid_counter_delta: bool,
    #[cfg(feature = "native-probe-observer")]
    progress_wall: phase_profile::ProgressTiming,
}
impl Trace {
    fn record(&mut self, mut value: Value) {
        // Preserve the first32 and a rolling tail of at most96 events, within
        // the unchanged aggregate stream bound. Oversized metadata is explicit.
        let mut bytes = value.to_string().len();
        if bytes > EVENT_BYTES {
            self.truncated = true;
            value = json!({"event":"metadata_summary_oversized"});
            bytes = value.to_string().len();
        }
        while self.events.len() >= EVENT_LIMIT || self.encoded_bytes + bytes > EVENT_TRACE_BYTES {
            self.truncated = true;
            // Prefix events cannot consume the entire byte bank: each is less
            // than1/33 of that bank, leaving room for the newest frontier.
            self.events.remove(PREFIX_EVENTS);
            self.encoded_bytes -= self.event_bytes.remove(PREFIX_EVENTS);
        }
        self.events.push(value);
        self.event_bytes.push(bytes);
        self.encoded_bytes += bytes;
    }
}
#[derive(Default)]
struct ContentCensus {
    rows: Vec<ContentIdentity>,
    path_bytes: usize,
    omitted: usize,
    path_overflow: bool,
}
struct ContentIdentity {
    demand: PathObservationDemand,
    file_ordinal: usize,
    first_attempt: usize,
    family: usize,
    seen: usize,
    new: usize,
    known: usize,
    paths_completed: usize,
}
impl ContentCensus {
    fn observe(
        &mut self,
        demand: &PathObservationDemand,
        family: usize,
        new: bool,
        attempt: usize,
    ) -> Option<usize> {
        if let Some(index) = self.rows.iter().position(|row| row.demand == *demand) {
            let row = &mut self.rows[index];
            row.seen += 1;
            row.new += usize::from(new);
            row.known += usize::from(!new);
            return Some(index);
        }
        let bytes = demand.path().as_path().as_os_str().as_encoded_bytes().len();
        if bytes > 4096 || self.path_bytes + bytes > CONTENT_PATH_BYTES {
            self.path_overflow = true;
            self.omitted += 1;
            return None;
        }
        if self.rows.len() == CONTENT_IDENTITIES {
            self.omitted += 1;
            return None;
        }
        let file_ordinal = self
            .rows
            .iter()
            .find(|row| {
                row.demand.namespace() == demand.namespace() && row.demand.path() == demand.path()
            })
            .map_or_else(
                || {
                    self.rows
                        .iter()
                        .map(|row| row.file_ordinal)
                        .max()
                        .unwrap_or(0)
                        + 1
                },
                |row| row.file_ordinal,
            );
        self.path_bytes += bytes;
        let index = self.rows.len();
        self.rows.push(ContentIdentity {
            demand: demand.clone(),
            file_ordinal,
            first_attempt: attempt,
            family,
            seen: 1,
            new: usize::from(new),
            known: usize::from(!new),
            paths_completed: 0,
        });
        Some(index)
    }
    fn completed_paths(&mut self, indices: &[usize]) {
        for index in indices {
            self.rows[*index].paths_completed += 1;
        }
    }
    fn summary(&self) -> Value {
        let pairs = self
            .rows
            .iter()
            .filter(|row| row.demand.operation() == PathObservationOperation::FileBytes)
            .filter(|row| {
                self.rows.iter().any(|other| {
                    other.file_ordinal == row.file_ordinal
                        && other.demand.operation() == PathObservationOperation::FileDigest
                })
            })
            .count();
        let rows = self.rows.iter().enumerate().map(|(index,row)| {
            let (namespace, instance) = match row.demand.namespace() {
                PathObservationNamespace::Host => ("host",None),
                PathObservationNamespace::Materialization(id) => ("materialization",Some(id.value())),
            };
            json!({"ordinal":index+1,"file_ordinal":row.file_ordinal,"first_attempt":row.first_attempt,"operation":if row.demand.operation()==PathObservationOperation::FileBytes {"file_bytes"} else {"file_digest"},"namespace":namespace,"instance":instance,"family":family_name(row.family),"seen":row.seen,"new":row.new,"known":row.known,"successful_paths":row.paths_completed})
        }).collect::<Vec<_>>();
        let result = json!({"complete":self.omitted==0 && !self.path_overflow,"omitted_occurrences":self.omitted,"path_overflow":self.path_overflow,"unique":self.rows.len(),"files":self.rows.iter().map(|row|row.file_ordinal).max().unwrap_or(0),"byte_digest_pairs":pairs,"repeated_need_occurrences":self.rows.iter().map(|row|row.seen-1).sum::<usize>(),"rows":rows});
        if result.to_string().len() <= CENSUS_BYTES {
            result
        } else {
            json!({"complete":false,"output_overflow":true})
        }
    }
}
#[derive(Clone, Copy, Default)]
struct ChargeTotals {
    calls: u64,
    source_attempts: u64,
    registry_attempts: u64,
    registry_bytes: u64,
    transferred_bytes: u64,
    decoded_bytes: u64,
    written_bytes: u64,
}
impl ChargeTotals {
    fn counters(command: &NativeDemandCommand<'_>) -> (Option<usize>, Self) {
        command
            .command_source_scope()
            .map_or((None, Self::default()), |scope| {
                let c = scope.snapshot();
                (
                    Some(Arc::as_ptr(scope) as usize),
                    Self {
                        calls: 0,
                        source_attempts: c.source_attempts,
                        registry_attempts: c.registry_attempts,
                        registry_bytes: c.registry_admitted_bytes,
                        transferred_bytes: c.transferred_bytes,
                        decoded_bytes: c.decoded_bytes,
                        written_bytes: c.written_bytes,
                    },
                )
            })
    }
    fn subtract(self, before: Self) -> Option<Self> {
        Some(Self {
            calls: 1,
            source_attempts: self.source_attempts.checked_sub(before.source_attempts)?,
            registry_attempts: self
                .registry_attempts
                .checked_sub(before.registry_attempts)?,
            registry_bytes: self.registry_bytes.checked_sub(before.registry_bytes)?,
            transferred_bytes: self
                .transferred_bytes
                .checked_sub(before.transferred_bytes)?,
            decoded_bytes: self.decoded_bytes.checked_sub(before.decoded_bytes)?,
            written_bytes: self.written_bytes.checked_sub(before.written_bytes)?,
        })
    }
    fn add(&mut self, value: Self) {
        self.calls += value.calls;
        self.source_attempts += value.source_attempts;
        self.registry_attempts += value.registry_attempts;
        self.registry_bytes += value.registry_bytes;
        self.transferred_bytes += value.transferred_bytes;
        self.decoded_bytes += value.decoded_bytes;
        self.written_bytes += value.written_bytes;
    }
    fn value(self) -> Value {
        json!({"calls":self.calls,"source_attempts":self.source_attempts,"registry_attempts":self.registry_attempts,"registry_bytes":self.registry_bytes,"transferred_bytes":self.transferred_bytes,"decoded_bytes":self.decoded_bytes,"written_bytes":self.written_bytes})
    }
}
struct PendingProgress {
    attempt: usize,
    scope_id: Option<usize>,
    before: ChargeTotals,
    family: usize,
    indices: [usize; CONTENT_IDENTITIES],
    count: usize,
}
fn family_name(family: usize) -> &'static str {
    [
        "none",
        "general",
        "source",
        "host",
        "legacy_or_unselected",
        "mixed",
        "non_path_or_refused",
    ][family]
}
fn content_family(command: &NativeDemandCommand<'_>, demand: &PathObservationDemand) -> usize {
    let PathObservationNamespace::Materialization(id) = demand.namespace() else {
        return 3;
    };
    if command
        .current_workspace_epoch()
        .transcripts()
        .unwrap_or(&[])
        .iter()
        .any(|t| t.reservation().instance() == id)
    {
        return 1;
    }
    if command
        .current_source_snapshot_epoch()
        .states()
        .unwrap_or(&[])
        .iter()
        .any(|state| state.reservation().instance() == id)
    {
        return 2;
    }
    4
}
fn progress_kind(result: &Result<NativeDemandProgress, NativeDemandSessionError>) -> usize {
    match result {
        Ok(NativeDemandProgress::Paths) => 0,
        Ok(NativeDemandProgress::Repositories) => 1,
        Ok(NativeDemandProgress::Environment) => 2,
        Ok(NativeDemandProgress::Downloads) => 3,
        Ok(NativeDemandProgress::Archives) => 4,
        Ok(NativeDemandProgress::Workspaces) => 5,
        Err(_) => 6,
    }
}
fn progress_name(kind: usize) -> &'static str {
    [
        "paths",
        "repositories",
        "environment",
        "downloads",
        "archives",
        "workspaces",
        "refused",
    ][kind]
}
fn text(value: &str) -> String {
    value.chars().take(512).collect()
}
fn operation(request: &RepositoryWorkspaceRequest) -> Value {
    let (kind, archive) = match request.operation() {
        RepositoryWorkspaceOperation::File { .. } => ("file", None),
        RepositoryWorkspaceOperation::Template { .. } => ("template", None),
        RepositoryWorkspaceOperation::Symlink { .. } => ("symlink", None),
        RepositoryWorkspaceOperation::Read(_) => ("read", None),
        RepositoryWorkspaceOperation::Exists(_) => ("exists", None),
        RepositoryWorkspaceOperation::Delete(_) => ("delete", None),
        RepositoryWorkspaceOperation::Execute(_) => ("execute", None),
        RepositoryWorkspaceOperation::Archive(effect) => (
            "archive",
            Some(json!({
                "format": format!("{:?}", effect.format()),
                "sha256": hex::encode(effect.sha256()),
                "url_count": effect.urls().len(),
                "strip_prefix": text(effect.strip_prefix()),
            })),
        ),
    };
    json!({"repository": text(request.reservation().repository().as_str()), "index": request.index(), "kind": kind, "archive": archive})
}
// Freeform errors are inspected privately under a fixed formatting bound.
// Only explicit static classifications leave this function; unknown stays
// unclassified regardless of source, URL, argv or secret-like message content.
#[derive(Default)]
struct Reason {
    text: String,
    truncated: bool,
}
impl std::fmt::Write for Reason {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let remaining = REASON_BYTES - self.text.len();
        if value.len() > remaining {
            let mut end = remaining;
            while !value.is_char_boundary(end) {
                end -= 1;
            }
            self.text.push_str(&value[..end]);
            self.truncated = true;
            return Err(std::fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}
fn reason(value: impl std::fmt::Display) -> &'static str {
    let mut writer = Reason::default();
    let _ = write!(&mut writer, "{value}");
    if writer.truncated {
        return "reason_format_limit";
    }
    if writer
        .text
        .contains("selected-registry BCR patch capture failed")
    {
        "bcr_patch_capture"
    } else {
        "unclassified"
    }
}
fn read_config(path: &Path) -> std::io::Result<Value> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((CONFIG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > CONFIG_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "diagnostic config exceeds fixed bound",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "diagnostic config JSON is invalid",
        )
    })
}
fn scope_summary(scope: Option<&Arc<source_service::SourceServiceScope>>) -> Option<Value> {
    scope.map(|s| {
        let c = s.snapshot();
        json!({"first_error":s.first_error_for_test().map(|e| format!("{e:?}")),"registry_attempts":c.registry_attempts,"registry_bytes":c.registry_admitted_bytes,"source_attempts":c.source_attempts,"transferred_bytes":c.transferred_bytes,"decoded_bytes":c.decoded_bytes,"written_bytes":c.written_bytes,"live_buffer_bytes":c.live_buffer_bytes,"peak_buffer_bytes":c.peak_buffer_bytes,"live_decoder_bytes":c.live_decoder_bytes,"peak_decoder_bytes":c.peak_decoder_bytes})
    })
}
fn materialization(request: &RepositoryMaterializationRequest) -> Value {
    use slug_bzlmod_v2::OverrideAttributeValue as V;
    let kind = match &request.kind {
        RepositoryMaterializationKind::Local { .. } => "local",
        RepositoryMaterializationKind::Immutable => "immutable",
        RepositoryMaterializationKind::GeneratedFileEffects(_) => "generated_file_effects",
        RepositoryMaterializationKind::GeneratedFilesystemEffects(_) => {
            "generated_filesystem_effects"
        }
        RepositoryMaterializationKind::GeneralWorkspace(_) => "general_workspace",
    };
    let attr = |name: &str| {
        request
            .repo_spec
            .attributes
            .iter()
            .find_map(|(key, value)| (key.as_str() == name).then_some(value))
    };
    let sha = match attr("sha256") {
        Some(V::String(s)) if s.len() == 64 && s.as_bytes().iter().all(u8::is_ascii_hexdigit) => {
            Some(s.as_str())
        }
        _ => None,
    };
    let url_count = match attr("urls").or_else(|| attr("url")) {
        Some(V::Iterable(rows)) => Some(rows.len()),
        Some(V::String(_)) => Some(1),
        _ => None,
    };
    let format = match attr("type") {
        Some(V::String(s)) => Some(text(s)),
        _ => None,
    };
    json!({"repository":text(request.id.canonical_repo.as_str()),"kind":kind,
        "rule_label":text(&request.repo_spec.rule_id.bzl_file.to_string()),
        "rule_function":text(&request.repo_spec.rule_id.rule_name),
        "declared_sha256":sha,"url_count":url_count,"declared_archive_format":format})
}
fn repository_error(error: &crate::runtime::repository_io::RepositorySessionError) -> Value {
    use crate::runtime::repository_io::RepositorySessionError as E;
    let kind = match error {
        E::Busy => "busy",
        E::StaleToken { .. } => "stale_token",
        E::TokenExhausted => "token_exhausted",
        E::InstanceExhausted => "instance_exhausted",
        E::WrongWorkspace => "wrong_workspace",
        E::KindMismatch => "kind_mismatch",
        E::ConflictingRequest(_) => "conflicting_request",
        E::NonSuccessSelection(_) => "non_success_selection",
        E::UnknownSelection(_) => "unknown_selection",
        E::DuplicateSelection(_) => "duplicate_selection",
        E::InvalidValidation(_) => "invalid_validation",
        E::ValidationAlreadyStarted => "validation_already_started",
        E::ValidationIncomplete => "validation_incomplete",
        E::InvalidRetainedRoot(_) => "invalid_retained_root",
        E::NativeObservation(_) => "native_observation",
        E::NativeWorkspace(_) => "native_workspace",
        E::InvalidImageAuthority => "invalid_image_authority",
        E::SourceService(_) => "source_service",
    };
    let detail = match error {
        E::SourceService(e) => Some(json!({"code":format!("{e:?}")})),
        E::NativeWorkspace(message) => Some(json!({"reason":reason(message)})),
        E::ConflictingRequest(repo)
        | E::NonSuccessSelection(repo)
        | E::UnknownSelection(repo)
        | E::DuplicateSelection(repo)
        | E::InvalidValidation(repo) => Some(json!({"repository":text(repo.as_str())})),
        _ => None,
    };
    json!({"kind":kind,"detail":detail})
}
fn needs_summary(needs: &SourcePreparationNeeds) -> Value {
    json!({
        "root_bootstrap": needs.root_module_bootstrap_request().is_some(),
        "paths": needs.path_observations().map_or(0, |n| n.demands().len()),
        "environment_names": needs.repository_environment().map_or(0, |n| n.names().len()),
        "materializations": needs.repository_materializations().len(),
        "materialization_rows": needs.repository_materializations().values().take(ROW_LIMIT).map(|r|materialization(r)).collect::<Vec<_>>(),
        "downloads": needs.selected_extension_downloads().len(),
        "legacy_executes": needs.repository_executes().len(),
        "legacy_filesystem": needs.repository_filesystem_operations().len(),
        "reservations": needs.workspace_reservations().len(),
        "operation_count": needs.workspace_operations().len(),
        "operations": needs.workspace_operations().values().take(ROW_LIMIT).map(|d| operation(d.request())).collect::<Vec<_>>(),
        "source_count": needs.source_snapshots().len(),
        "sources": needs.source_snapshots().values().take(ROW_LIMIT).map(|r| json!({"repository":text(r.repository().as_str()),"kind":format!("{:?}",r.kind()),"materialization":r.materialization_request().map(|r|materialization(r))})).collect::<Vec<_>>(),
    })
}
fn native_error(error: &NativeDemandSessionError) -> Value {
    use crate::runtime::repository_physical_root::PhysicalRootError as P;
    match error {
        NativeDemandSessionError::SourceService(e) => {
            json!({"kind":"source_service","code":format!("{e:?}")})
        }
        NativeDemandSessionError::RepositoryWorkspace(e) => {
            let detail = match e {
                P::SourceService(e) => json!({"kind":"source_service","code":format!("{e:?}")}),
                P::Protocol(e) => json!({"kind":"protocol","code":format!("{e:?}")}),
                P::Io(e) => {
                    json!({"kind":"io","io_kind":format!("{:?}",e.kind()),"errno":e.raw_os_error()})
                }
                P::Mutation(e) => {
                    json!({"kind":"mutation","stage":format!("{:?}",e.stage),"reason":reason(&e.message)})
                }
                P::Reservation => json!("reservation"),
                P::Mode => json!("mode"),
                P::Backing => json!("backing"),
                P::RootIdentity => json!("root_identity"),
                P::Snapshot => json!("snapshot"),
                P::UnsupportedOperation => json!("unsupported_operation"),
                P::Resource => json!("resource"),
                P::Inactive => json!("inactive"),
                P::Observation(_) => json!("observation"),
            };
            json!({"kind":"repository_workspace","detail":detail})
        }
        NativeDemandSessionError::Repository(e) => {
            json!({"kind":"repository","detail":repository_error(e)})
        }
        NativeDemandSessionError::Computation(e) => {
            json!({"kind":"computation","reason":reason(e)})
        }
        NativeDemandSessionError::DownloadCapture(message) => {
            json!({"kind":"download_capture","reason":reason(message)})
        }
        NativeDemandSessionError::RepositoryFilesystem(e) => {
            json!({"kind":"repository_filesystem","stage":format!("{:?}",e.stage),"reason":reason(&e.message)})
        }
        NativeDemandSessionError::ObservedTerminal(e) => {
            json!({"kind":"observed_terminal","code":format!("{e:?}")})
        }
        NativeDemandSessionError::LegacyRepositoryExecuteUnsupported => {
            json!({"kind":"legacy_execute_unsupported"})
        }
        _ => json!({"kind":"other_driver","variant":format!("{:?}",std::mem::discriminant(error))}),
    }
}
fn terminal_error(error: &CqueryCommandError) -> &'static str {
    match error {
        CqueryCommandError::MissingTarget { .. } => "missing_target",
        CqueryCommandError::Evaluation(_) => "evaluation",
        CqueryCommandError::ExecutableRuleMissingExecutable(_) => {
            "executable_rule_missing_executable"
        }
        CqueryCommandError::Analysis(_) => "analysis",
        CqueryCommandError::Request(_) => "request",
        CqueryCommandError::Infrastructure(_) => "infrastructure",
    }
}

// Discriminants only. Keep this separate from the existing private, bounded
// reason formatter; opaque payloads are never inspected by this projection.
fn analysis_terminal_leaf(kind: &AnalysisErrorKind) -> &'static str {
    match kind {
        AnalysisErrorKind::TargetNotFound { .. } => "analysis_target_not_found",
        AnalysisErrorKind::ExecutableRuleMissingExecutable { .. } => "analysis_executable_missing",
        AnalysisErrorKind::Message(_) => "analysis_message_opaque",
        AnalysisErrorKind::Registration(_) => "analysis_registration_opaque",
    }
}

fn terminal_leaf(error: &CqueryCommandError) -> &'static str {
    match error {
        CqueryCommandError::MissingTarget { .. } => "command_missing_target",
        CqueryCommandError::Evaluation(_) => "command_evaluation_opaque",
        CqueryCommandError::ExecutableRuleMissingExecutable(error)
        | CqueryCommandError::Analysis(error) => analysis_terminal_leaf(error.kind()),
        CqueryCommandError::Request(_) => "command_request_opaque",
        CqueryCommandError::Infrastructure(_) => "command_infrastructure_opaque",
    }
}

#[cfg(feature = "native-probe-observer")]
fn file_bytes_discovery_report(before: bool, after: bool) -> Value {
    json!({
        "before": before,
        "after": after,
        "extended_during_capture": if before { None } else { Some(after) },
        "process_wide": true,
        "request_isolated": false,
        "is_servicing_evidence": false,
    })
}
#[derive(Clone)]
struct ObservedCquery {
    inner: CqueryCommandRoot,
    trace: Arc<Mutex<Trace>>,
}
#[async_trait]
impl NativeCommandRoot for ObservedCquery {
    type Terminal = CqueryCommandTerminal;
    fn progress_observed(
        &self,
        needs: &SourcePreparationNeeds,
        command: &NativeDemandCommand<'_>,
        attempt: usize,
        result: Option<&Result<NativeDemandProgress, NativeDemandSessionError>>,
    ) {
        // Sample the progress boundary before census/counter rendering or locks.
        #[cfg(feature = "native-probe-observer")]
        let progress_end = result.map(|_| Instant::now());
        let mut trace = self.trace.lock().unwrap();
        if let Some(result) = result {
            #[cfg(feature = "native-probe-observer")]
            trace
                .progress_wall
                .finish(attempt, progress_kind(result), progress_end.unwrap());
            let pending = trace
                .pending_progress
                .take()
                .expect("paired progress observation");
            let (scope_id, after) = ChargeTotals::counters(command);
            let kind = progress_kind(result);
            if pending.attempt != attempt || pending.scope_id != scope_id {
                trace.invalid_counter_delta = true;
            } else if let Some(delta) = after.subtract(pending.before) {
                let family = if kind == 0 { pending.family } else { 6 };
                trace.progress_totals[kind].add(delta);
                trace.family_totals[family].add(delta);
                if kind == 0 {
                    trace
                        .census
                        .completed_paths(&pending.indices[..pending.count]);
                }
                trace.record(json!({"event":"progress_observed","attempt":attempt,"kind":progress_name(kind),"family":family_name(family),"before":pending.before.value(),"after":after.value(),"delta":delta.value()}));
            } else {
                trace.invalid_counter_delta = true;
            }
        } else {
            assert!(trace.pending_progress.is_none());
            let (scope_id, before) = ChargeTotals::counters(command);
            let mut pending = PendingProgress {
                attempt,
                scope_id,
                before,
                family: 0,
                indices: [0; CONTENT_IDENTITIES],
                count: 0,
            };
            if let Some(paths) = needs.path_observations() {
                for demand in paths.demands() {
                    // These two content variants carry no image/Windows owners.
                    if demand.image_path().is_some() || demand.windows_long_path_input().is_some() {
                        continue;
                    }
                    if !matches!(
                        demand.operation(),
                        PathObservationOperation::FileBytes | PathObservationOperation::FileDigest
                    ) {
                        continue;
                    }
                    let new = command.path_observations.get(demand).is_none();
                    let family = content_family(command, demand);
                    if new {
                        pending.family = if pending.family == 0 || pending.family == family {
                            family
                        } else {
                            5
                        };
                    }
                    if let Some(index) = trace.census.observe(demand, family, new, attempt) {
                        if new && pending.count < CONTENT_IDENTITIES {
                            pending.indices[pending.count] = index;
                            pending.count += 1;
                        }
                    }
                }
            }
            trace.pending_progress = Some(pending);
        }
        drop(trace);
        self.inner
            .progress_observed(needs, command, attempt, result);
        // Start last, after diagnostics and unchanged inner delegation. No lock
        // remains held through actual progress; only fixed dispatch tail remains.
        #[cfg(feature = "native-probe-observer")]
        if result.is_none() {
            self.trace.lock().unwrap().progress_wall.begin(attempt);
        }
    }
    fn progress_refused(
        &self,
        needs: &SourcePreparationNeeds,
        command: &NativeDemandCommand<'_>,
        attempt: usize,
        error: &NativeDemandSessionError,
    ) {
        let scope = scope_summary(command.command_source_scope());
        let mut trace = self.trace.lock().unwrap();
        let epoch = trace.latest_epoch.clone();
        trace.record(json!({"event":"progress_refused","attempt":attempt,"error":native_error(error),"needs":needs_summary(needs),"scope":scope,"epoch":epoch}));
        drop(trace);
        self.inner.progress_refused(needs, command, attempt, error);
    }
    fn complete<'a>(
        &'a self,
        terminal: &'a mut Self::Terminal,
        context: NativeCommandCompletionContext<'a, '_>,
    ) -> impl Future<Output = Result<(), NativeDemandSessionError>> + 'a {
        let scope = context.command.command_source_scope().cloned();
        let before = scope_summary(scope.as_ref());
        async move {
            // Delegate with no instrumentation lock across its future.
            let result = self.inner.complete(terminal, context).await;
            let after = scope_summary(scope.as_ref());
            self.trace.lock().unwrap().record(json!({
                "event":"terminal_completion", "scope_before":before, "scope_after":after,
                "result":result.as_ref().err().map(native_error),
            }));
            result
        }
    }
    fn publish(&self, terminal: &mut Self::Terminal) -> std::io::Result<()> {
        self.inner.publish(terminal)
    }
    fn publication_completed(&self, terminal: &Self::Terminal) -> bool {
        self.inner.publication_completed(terminal)
    }
    fn initializes_request_revision(&self) -> bool {
        self.inner.initializes_request_revision()
    }
    fn requires_workspace_status_inputs(&self) -> bool {
        self.inner.requires_workspace_status_inputs()
    }
    fn source_certificate<'a>(
        &self,
        terminal: &'a Self::Terminal,
    ) -> Option<&'a SourceCertificate> {
        self.inner.source_certificate(terminal)
    }
    fn observations<'a>(&self, terminal: &'a Self::Terminal) -> Option<&'a PathObservationEpoch> {
        self.inner.observations(terminal)
    }
    fn observed_selection_association(&self) -> ObservedSelectionAssociation {
        self.inner.observed_selection_association()
    }
    fn terminal_observed_selection_association(
        &self,
        terminal: &Self::Terminal,
    ) -> ObservedSelectionAssociation {
        self.inner.terminal_observed_selection_association(terminal)
    }
    fn terminal_demand_association(&self, terminal: &Self::Terminal) -> TerminalDemandAssociation {
        self.inner.terminal_demand_association(terminal)
    }
    fn event_reconciliation_policy(&self, terminal: &Self::Terminal) -> EventReconciliationPolicy {
        self.inner.event_reconciliation_policy(terminal)
    }
    fn allows_empty_terminal(&self) -> bool {
        self.inner.allows_empty_terminal()
    }
    fn allows_unavailable_terminal_roots(&self, terminal: &Self::Terminal) -> bool {
        self.inner.allows_unavailable_terminal_roots(terminal)
    }
    async fn compute(
        &self,
        transaction: &mut dice::DiceTransaction,
    ) -> Result<SourcePreparationOutcome<Self::Terminal>, NativeDemandSessionError> {
        // No instrumentation lock or borrowed userdata survives genuine compute.
        let epoch_summary = {
            let epoch = transaction
                .per_transaction_data()
                .data
                .get::<slug_bzlmod_v2::RepositoryWorkspaceEpoch>()
                .unwrap();
            let transcripts = epoch.transcripts().unwrap_or(&[]);
            json!({"roots":transcripts.len(),"record_count":transcripts.iter().map(|t|t.records().len()).sum::<usize>(),"latest":transcripts.iter().rev().filter_map(|t|t.records().last()).take(ROW_LIMIT).map(|r|operation(r.request())).collect::<Vec<_>>()})
        };
        let outcome = self.inner.compute(transaction).await;
        let summary = match &outcome {
            Ok(SourcePreparationOutcome::Need(needs)) => {
                json!({"outcome":"need","needs":needs_summary(needs)})
            }
            Ok(SourcePreparationOutcome::Complete(terminal)) => match terminal.result.as_ref() {
                Ok(evaluation) => {
                    json!({"outcome":"complete_success","analyses":evaluation.analyses().count()})
                }
                Err(error) => {
                    json!({"outcome":"complete_error","kind":terminal_error(error),"reason":reason(error)})
                }
            },
            Err(error) => json!({"outcome":"compute_error","error":native_error(error)}),
        };
        let mut trace = self.trace.lock().unwrap();
        trace.latest_epoch = epoch_summary.clone();
        trace.computes += 1;
        let compute = trace.computes;
        trace.record(
            json!({"event":"compute","compute":compute,"epoch":epoch_summary,"result":summary}),
        );
        outcome
    }
}
fn verify_inputs(config: &Value, workspace: &Path) {
    let files = config["files"].as_array().unwrap();
    assert_eq!(files.len(), OWNER_FILES.len());
    let original = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/v2_oracle/fixtures/rules-rust-073-toolchain-owner/workspace");
    for relative in OWNER_FILES {
        let row = files
            .iter()
            .find(|r| r["relative"].as_str() == Some(relative))
            .unwrap();
        let actual = std::fs::read(workspace.join(relative)).unwrap();
        let expected = std::fs::read(original.join(relative)).unwrap();
        assert!(actual == expected, "staged original differs: {relative}");
        assert_eq!(actual.len() as u64, row["bytes"].as_u64().unwrap());
        assert_eq!(
            hex::encode(sha2::Sha256::digest(&actual)),
            row["sha256"].as_str().unwrap()
        );
    }
}
#[test]
#[ignore = "one genuine original Rust cquery diagnostic; source-only staged input JSON and finite native supervisor required"]
fn native_original_rust_toolchain_cquery_frontier() {
    #[cfg(feature = "native-probe-observer")]
    let file_bytes_before = slug_loading_v2::file_bytes_lookahead_extended();
    let config_path =
        std::env::var_os("SLUG_NATIVE_RUST_INPUTS").expect("source-only stager config required");
    let config = read_config(Path::new(&config_path)).unwrap();
    let workspace = Path::new(config["workspace"].as_str().unwrap());
    assert!(workspace.is_absolute());
    verify_inputs(&config, workspace);
    let mut runtime =
        WorkspaceRuntime::new(workspace, crate::runtime::ProcessHostOwner::native()).unwrap();
    #[cfg(feature = "native-probe-observer")]
    let phase_profile = phase_profile::InstalledProfile::attach(&mut runtime).unwrap();
    assert!(runtime.source_scope().is_none());
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();
    let trace = Arc::new(Mutex::new(Trace::default()));
    let host = runtime.process_host.default_configuration_inputs().unwrap();
    let root = ObservedCquery {
        inner: CqueryCommandRoot {
            expression: QueryExpression::parse("//pkg:probe").unwrap(),
            workspace: NormalizedAbsolutePath::new(runtime.workspace.clone()).unwrap(),
            base_configuration: ConfigurationKey::from_slug(
                SlugConfiguration::default_target(&host).unwrap(),
            ),
            configuration_overlay: CommandConfigurationOverlay::from(vec![
                CommandConfigurationOccurrence::starlark(
                    "@rules_rust//rust/toolchain/channel",
                    Some("nightly"),
                    false,
                ),
            ]),
            roots: Arc::from([CqueryRootTarget {
                requested: Arc::from("//pkg:probe"),
                canonical: CanonicalLabel::parse("@@//pkg:probe").unwrap(),
            }]),
            literal_roots: Arc::from([(Arc::from("//pkg:probe"), 0)]),
            include_implicit: true,
            include_tool: true,
        },
        trace: trace.clone(),
    };
    let distdirs = config["distdirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    let inputs = NativeDemandRequestInputBundle {
        archive_distdirs: crate::runtime::ArchiveDistdirPolicy::from_request(workspace, &distdirs)
            .unwrap(),
        runtime_sources: None,
        guest_namespace: None,
        command_policy: BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
        environment_policy: BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None)
            .unwrap(),
        lockfile_mode: LockfileMode::Off,
        registry_urls: RegistryUrls::from_request(
            workspace,
            &[config["registry_uri"].as_str().unwrap().to_owned()],
        )
        .unwrap(),
        repository_environment: slug_bzlmod_v2::RepositoryEnvironmentSnapshot::from_canonical([
            RepositoryEnvironmentEntry::new("CC", "/usr/bin/gcc"),
            RepositoryEnvironmentEntry::new("PATH", "/usr/bin:/bin"),
        ])
        .unwrap(),
    };
    let outcome = runtime.drive_command(inputs, root);
    let status = match &outcome {
        Err(error) => json!({"status":"driver_refusal","error":native_error(error)}),
        Ok(driven) => match driven.accepted.terminal_for_test().result.as_ref() {
            Err(error) => {
                json!({"status":"accepted_terminal_error","kind":terminal_error(error),"terminal_leaf":terminal_leaf(error),"reason":reason(error),"attempts":driven.attempts})
            }
            Ok(evaluation) => {
                json!({"status":"accepted_success","analyses":evaluation.analyses().count(),"attempts":driven.attempts})
            }
        },
    };
    let snapshot = resources.snapshot();
    let mut report = {
        let trace = trace.lock().unwrap();
        json!({"result":status,"computes":trace.computes,"truncated":trace.truncated,"events":trace.events,"content_census":trace.census.summary(),"invalid_counter_delta":trace.invalid_counter_delta,"progress_totals":trace.progress_totals.iter().enumerate().map(|(kind,total)|json!({"kind":progress_name(kind),"counters":total.value()})).collect::<Vec<_>>(),"family_totals":trace.family_totals.iter().enumerate().map(|(family,total)|json!({"family":family_name(family),"counters":total.value()})).collect::<Vec<_>>(),"backing_bytes":snapshot.backing_bytes,"peak_backing_bytes":snapshot.peak_backing_bytes,"native_fds":snapshot.native_fds,"peak_native_fds":snapshot.peak_native_fds,"retained_bytes":snapshot.retained_bytes,"peak_retained_bytes":snapshot.peak_retained_bytes})
    };
    drop(outcome);
    drop(runtime);
    let final_snapshot = resources.snapshot();
    assert_eq!(final_snapshot.native_fds, 0);
    assert_eq!(final_snapshot.backing_bytes, 0);
    assert_eq!(final_snapshot.retained_bytes, baseline);
    #[cfg(feature = "native-probe-observer")]
    {
        report["file_bytes_discovery"] = file_bytes_discovery_report(
            file_bytes_before,
            slug_loading_v2::file_bytes_lookahead_extended(),
        );
        report["phase_profile"] = phase_profile.finish();
        report["progress_wall"] = trace.lock().unwrap().progress_wall.summary();
        assert!(
            report["phase_profile"].to_string().len() + report["progress_wall"].to_string().len()
                < PHASE_BYTES
        );
    }
    verify_inputs(&config, workspace);
    let cleanup = json!({"native_fds":final_snapshot.native_fds,"backing_bytes":final_snapshot.backing_bytes,"retained_bytes":final_snapshot.retained_bytes,"baseline":baseline}).to_string();
    let overhead = "WP425_RUST_FRONTIER ".len() + "WP425_RUST_CLEANUP ".len() + 2 + cleanup.len();
    let mut encoded = report.to_string();
    while encoded.len().checked_add(overhead).unwrap() > TRACE_BYTES {
        let events = report["events"].as_array_mut().unwrap();
        assert!(events.len() > PREFIX_EVENTS);
        events.remove(PREFIX_EVENTS);
        report["truncated"] = json!(true);
        encoded = report.to_string();
    }
    assert!(encoded.len() + overhead <= TRACE_BYTES);
    println!("WP425_RUST_FRONTIER {encoded}");
    println!("WP425_RUST_CLEANUP {cleanup}");
}

#[test]
#[cfg(feature = "native-probe-observer")]
fn file_bytes_discovery_marker_qualifies_preexisting_true_without_counts_or_payloads() {
    for (before, after, expected) in [
        (false, false, json!(false)),
        (false, true, json!(true)),
        (true, true, Value::Null),
        (true, false, Value::Null),
    ] {
        let report = file_bytes_discovery_report(before, after);
        assert_eq!(report["extended_during_capture"], expected);
        assert_eq!(report["process_wide"], true);
        assert_eq!(report["request_isolated"], false);
        assert_eq!(report["is_servicing_evidence"], false);
        assert!(report.to_string().len() <= 256);
    }
    assert!(EVENT_TRACE_BYTES + CENSUS_BYTES + PHASE_BYTES + 96 + 256 < TRACE_BYTES);
}

#[test]
fn frontier_terminal_discriminants_never_visit_private_payloads() {
    for payload in [
        "private-payload".to_owned(),
        "private-payload\0界".repeat(REASON_BYTES),
    ] {
        let label = CanonicalLabel::parse("@@//pkg:probe").unwrap();
        let build_file = std::path::PathBuf::from(&payload);
        let cases = [
            (
                CqueryCommandError::MissingTarget {
                    requested: Arc::from(payload.as_str()),
                    label: label.clone(),
                    build_file: build_file.clone(),
                },
                "command_missing_target",
            ),
            (
                CqueryCommandError::Evaluation(Arc::from(payload.as_str())),
                "command_evaluation_opaque",
            ),
            (
                CqueryCommandError::ExecutableRuleMissingExecutable(
                    slug_analysis_v2::AnalysisError::message(&payload),
                ),
                "analysis_message_opaque",
            ),
            (
                CqueryCommandError::Analysis(slug_analysis_v2::AnalysisError::message(&payload)),
                "analysis_message_opaque",
            ),
            (
                CqueryCommandError::Request(Arc::from(payload.as_str())),
                "command_request_opaque",
            ),
            (
                CqueryCommandError::Infrastructure(Arc::from(payload.as_str())),
                "command_infrastructure_opaque",
            ),
        ];
        for (error, expected) in cases {
            let leaf = terminal_leaf(&error);
            assert_eq!(leaf, expected);
            let encoded = json!({"terminal_leaf":leaf}).to_string();
            assert!(encoded.len() <= 96);
            assert!(!encoded.contains("private-payload"));
        }
        assert_eq!(
            analysis_terminal_leaf(&AnalysisErrorKind::TargetNotFound { label, build_file }),
            "analysis_target_not_found"
        );
        assert_eq!(
            analysis_terminal_leaf(&AnalysisErrorKind::ExecutableRuleMissingExecutable {
                rule_class: payload.into()
            }),
            "analysis_executable_missing"
        );
    }
    // The existing aggregate reserve remains unchanged. This finite field
    // consumes at most96 bytes of its fixed metadata allowance.
    assert!(EVENT_TRACE_BYTES + CENSUS_BYTES + PHASE_BYTES + 96 < TRACE_BYTES);
}

#[test]
fn frontier_diagnostic_reason_and_config_are_bounded_and_never_emit_payload() {
    let payload = "arbitrary source URL argv authorization private-payload";
    assert_eq!(reason(payload), "unclassified");
    assert_eq!(
        reason(format_args!(
            "{payload} selected-registry BCR patch capture failed {payload}"
        )),
        "bcr_patch_capture"
    );
    struct Large;
    impl std::fmt::Display for Large {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for _ in 0..=REASON_BYTES {
                f.write_str("x")?;
            }
            Ok(())
        }
    }
    assert_eq!(reason(Large), "reason_format_limit");
    let mut writer = Reason::default();
    assert!(write!(&mut writer, "{}", Large).is_err());
    assert_eq!(writer.text.len(), REASON_BYTES);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    std::fs::write(&path, format!("{{}}{}", " ".repeat(CONFIG_BYTES - 2))).unwrap();
    assert!(read_config(&path).is_ok());
    std::fs::write(&path, "x".repeat(CONFIG_BYTES + 1)).unwrap();
    assert_eq!(
        read_config(&path).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    let mut trace = Trace::default();
    for n in 0..200 {
        trace.record(json!({"n":n}));
    }
    assert_eq!(trace.events.len(), EVENT_LIMIT);
    assert_eq!(trace.events[0]["n"], 0);
    assert_eq!(trace.events[PREFIX_EVENTS - 1]["n"], PREFIX_EVENTS - 1);
    assert_eq!(trace.events[PREFIX_EVENTS]["n"], 104);
    assert_eq!(trace.events[EVENT_LIMIT - 1]["n"], 199);
    assert!(trace.encoded_bytes <= TRACE_BYTES);
    let demand = |name: &str, op| {
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(format!("/private-census/{name}")).unwrap(),
            op,
        )
    };
    let bytes = demand("same", PathObservationOperation::FileBytes);
    let digest = demand("same", PathObservationOperation::FileDigest);
    let mut census = ContentCensus::default();
    assert_eq!(census.observe(&bytes, 3, true, 1), Some(0));
    assert_eq!(census.observe(&bytes, 3, true, 2), Some(0));
    assert_eq!(census.observe(&bytes, 3, false, 3), Some(0));
    assert_eq!(census.observe(&digest, 3, true, 4), Some(1));
    census.completed_paths(&[0, 1]);
    let summary = census.summary();
    assert_eq!(summary["unique"], 2);
    assert_eq!(summary["files"], 1);
    assert_eq!(summary["byte_digest_pairs"], 1);
    assert_eq!(summary["rows"][0]["new"], 2);
    assert_eq!(summary["rows"][0]["known"], 1);
    assert_eq!(summary["rows"][0]["successful_paths"], 1);
    assert!(!summary.to_string().contains("private-census"));
    for n in 0..CONTENT_IDENTITIES - 2 {
        assert!(
            census
                .observe(
                    &demand(&format!("new-{n}"), PathObservationOperation::FileBytes),
                    3,
                    true,
                    n + 5
                )
                .is_some()
        );
    }
    assert!(
        census
            .observe(
                &demand("over-cap", PathObservationOperation::FileBytes),
                3,
                true,
                99
            )
            .is_none()
    );
    assert_eq!(census.rows.len(), CONTENT_IDENTITIES);
    assert_eq!(census.summary()["complete"], false);
    let mut bounded = ContentCensus::default();
    let long = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(format!("/{}", "p".repeat(4096))).unwrap(),
        PathObservationOperation::FileBytes,
    );
    assert!(bounded.observe(&long, 3, true, 1).is_none());
    assert!(bounded.path_overflow);
    assert!(bounded.rows.is_empty());
    let before = ChargeTotals {
        source_attempts: 63,
        ..ChargeTotals::default()
    };
    let after = ChargeTotals {
        source_attempts: 64,
        ..ChargeTotals::default()
    };
    assert_eq!(after.subtract(before).unwrap().source_attempts, 1);
    assert_eq!(after.subtract(after).unwrap().source_attempts, 0);
    assert!(before.subtract(after).is_none());
}
