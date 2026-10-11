use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use slug_workspace_v2::NormalizedAbsolutePath;

#[cfg(all(test, target_os = "linux"))]
use super::buffer_provenance::SourceBufferKind;
#[cfg(all(test, target_os = "linux"))]
use super::buffer_provenance::SourceBufferOrigin;
use super::buffer_provenance::SourceBufferSite;
#[cfg(all(test, target_os = "linux"))]
use super::buffer_provenance::SourceBufferSnapshot;
#[cfg(all(test, target_os = "linux"))]
use super::buffer_provenance::SourceBufferState;
use super::buffer_provenance::SourceBufferUnit;
use super::timeout_provenance::SourceTimeoutDeadline;
use super::timeout_provenance::SourceTimeoutOrigin;
use super::timeout_provenance::SourceTimeoutPhase;
use super::timeout_provenance::SourceTimeoutReturn;
use super::timeout_provenance::SourceTimeoutSite;
#[cfg(all(test, target_os = "linux"))]
use super::timeout_provenance::SourceTimeoutSnapshot;
#[cfg(all(test, target_os = "linux"))]
use super::timeout_provenance::SourceTimeoutState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SourceCounter {
    RegistryAttempts,
    RegistryBytes,
    SourceAttempts,
    TransferBytes,
    DecodedBytes,
    WrittenBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct SourceServiceLimits {
    /// Fixed operational policy for this scoped diagnostic owner.
    pub(in crate::runtime) allow_network: bool,
    pub(in crate::runtime) registry_object_bytes: u64,
    pub(in crate::runtime) registry_attempts: u64,
    pub(in crate::runtime) registry_admitted_bytes: u64,
    pub(in crate::runtime) source_attempts: u64,
    pub(in crate::runtime) transferred_bytes: u64,
    pub(in crate::runtime) decoded_bytes: u64,
    pub(in crate::runtime) written_bytes: u64,
    pub(in crate::runtime) live_buffer_bytes: usize,
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) blocking_threads: Option<usize>,
}
impl Default for SourceServiceLimits {
    fn default() -> Self {
        Self {
            allow_network: true,
            registry_object_bytes: 1 << 20,
            registry_attempts: 128,
            registry_admitted_bytes: 128 << 20,
            source_attempts: 64,
            transferred_bytes: 2 << 30,
            decoded_bytes: 2 << 30,
            written_bytes: 2 << 30,
            live_buffer_bytes: 16 << 20,
            #[cfg(all(test, target_os = "linux"))]
            blocking_threads: None,
        }
    }
}
impl SourceServiceLimits {
    pub(super) const MAX_REGISTRY_ATTEMPTS: u64 = 256;

    #[cfg(all(test, target_os = "linux"))]
    pub(super) const MAX_SOURCE_ATTEMPTS: u64 = 512;
    #[cfg(not(all(test, target_os = "linux")))]
    pub(super) const MAX_SOURCE_ATTEMPTS: u64 = 64;

    // The explicit actual-root diagnostic admits one existing 32 MiB payload
    // plus charged recorder/scratch. Production keeps the original 16 MiB cap.
    #[cfg(all(test, target_os = "linux"))]
    pub(super) const MAX_LIVE_BUFFER_BYTES: usize = 40 << 20;
    #[cfg(not(all(test, target_os = "linux")))]
    pub(super) const MAX_LIVE_BUFFER_BYTES: usize = 16 << 20;

    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn actual_root() -> Self {
        Self {
            registry_attempts: Self::MAX_REGISTRY_ATTEMPTS,
            live_buffer_bytes: Self::MAX_LIVE_BUFFER_BYTES,
            blocking_threads: Some(2),
            ..Self::default()
        }
    }

    fn valid(self) -> bool {
        #[cfg(all(test, target_os = "linux"))]
        if self.blocking_threads.is_some_and(|n| !(1..=2).contains(&n)) {
            return false;
        }
        let max = Self::default();
        [
            (self.registry_object_bytes, max.registry_object_bytes),
            (self.registry_attempts, Self::MAX_REGISTRY_ATTEMPTS),
            (self.registry_admitted_bytes, max.registry_admitted_bytes),
            (self.source_attempts, Self::MAX_SOURCE_ATTEMPTS),
            (self.transferred_bytes, max.transferred_bytes),
            (self.decoded_bytes, max.decoded_bytes),
            (self.written_bytes, max.written_bytes),
        ]
        .into_iter()
        .all(|(v, m)| v > 0 && v <= m)
            && self.live_buffer_bytes > 0
            && self.live_buffer_bytes <= Self::MAX_LIVE_BUFFER_BYTES
    }
    fn maximum(self, counter: SourceCounter) -> u64 {
        match counter {
            SourceCounter::RegistryAttempts => self.registry_attempts,
            SourceCounter::RegistryBytes => self.registry_admitted_bytes,
            SourceCounter::SourceAttempts => self.source_attempts,
            SourceCounter::TransferBytes => self.transferred_bytes,
            SourceCounter::DecodedBytes => self.decoded_bytes,
            SourceCounter::WrittenBytes => self.written_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SourceServiceError {
    InvalidLimits,
    ForeignScope,
    AcquisitionClosed,
    NetworkDisabled,
    Cancelled,
    WorkDeadline,
    CleanupDeadline,
    Limit(SourceCounter),
    Overflow(SourceCounter),
    BufferLimit,
    Allocation,
    UnsupportedSourceOwner,
}
impl std::fmt::Display for SourceServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source service refused: {self:?}")
    }
}
impl std::error::Error for SourceServiceError {}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::runtime) struct SourceServiceCounters {
    pub(in crate::runtime) registry_attempts: u64,
    pub(in crate::runtime) registry_admitted_bytes: u64,
    pub(in crate::runtime) source_attempts: u64,
    pub(in crate::runtime) transferred_bytes: u64,
    pub(in crate::runtime) decoded_bytes: u64,
    pub(in crate::runtime) written_bytes: u64,
    pub(in crate::runtime) live_buffer_bytes: usize,
    pub(in crate::runtime) peak_buffer_bytes: usize,
}
impl SourceServiceCounters {
    fn slot(&mut self, counter: SourceCounter) -> &mut u64 {
        match counter {
            SourceCounter::RegistryAttempts => &mut self.registry_attempts,
            SourceCounter::RegistryBytes => &mut self.registry_admitted_bytes,
            SourceCounter::SourceAttempts => &mut self.source_attempts,
            SourceCounter::TransferBytes => &mut self.transferred_bytes,
            SourceCounter::DecodedBytes => &mut self.decoded_bytes,
            SourceCounter::WrittenBytes => &mut self.written_bytes,
        }
    }
}
#[derive(Default)]
struct ScopeState {
    counters: SourceServiceCounters,
    first: Option<SourceServiceError>,
    closed: bool,
    #[cfg(all(test, target_os = "linux"))]
    timeout: SourceTimeoutState,
    #[cfg(all(test, target_os = "linux"))]
    buffer: SourceBufferState,
    #[cfg(all(test, target_os = "linux"))]
    census: Option<super::source_charge_census::CensusState>,
}

pub(in crate::runtime) struct SourceServiceScope {
    workspace: NormalizedAbsolutePath,
    work_deadline: Instant,
    cleanup_deadline: Instant,
    limits: SourceServiceLimits,
    cancelled: tokio::sync::watch::Sender<bool>,
    state: Mutex<ScopeState>,
    #[cfg(all(test, target_os = "linux"))]
    registry_association: std::sync::OnceLock<std::sync::Weak<super::RegistryRequestAssociation>>,
    #[cfg(all(test, target_os = "linux"))]
    known_shas_root: Option<NormalizedAbsolutePath>,
    #[cfg(all(test, target_os = "linux"))]
    offline_archives: [Option<Arc<super::OfflineArchiveBacking>>; 2],
}
impl SourceServiceScope {
    pub(super) fn new(
        workspace: NormalizedAbsolutePath,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
    ) -> Result<Arc<Self>, SourceServiceError> {
        let now = Instant::now();
        if !limits.valid()
            || work_deadline <= now
            || work_deadline > now + Duration::from_secs(27)
            || cleanup_deadline < work_deadline
            || cleanup_deadline
                > work_deadline
                    .checked_add(Duration::from_secs(3))
                    .ok_or(SourceServiceError::InvalidLimits)?
        {
            return Err(SourceServiceError::InvalidLimits);
        }
        Ok(Arc::new(Self {
            workspace,
            work_deadline,
            cleanup_deadline,
            limits,
            cancelled: tokio::sync::watch::channel(false).0,
            state: Mutex::new(ScopeState::default()),
            #[cfg(all(test, target_os = "linux"))]
            registry_association: std::sync::OnceLock::new(),
            #[cfg(all(test, target_os = "linux"))]
            known_shas_root: None,
            #[cfg(all(test, target_os = "linux"))]
            offline_archives: [None, None],
        }))
    }
    #[cfg(test)]
    pub(in crate::runtime) fn new_for_test(
        workspace: NormalizedAbsolutePath,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
    ) -> Result<Arc<Self>, SourceServiceError> {
        Self::new(workspace, work_deadline, cleanup_deadline, limits)
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn new_for_test_with_known_shas_allowance(
        workspace: NormalizedAbsolutePath,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
        selected_rules_rust_root: NormalizedAbsolutePath,
    ) -> Result<Arc<Self>, SourceServiceError> {
        Self::new_for_test_with_offline_archive(
            workspace,
            work_deadline,
            cleanup_deadline,
            limits,
            Some(selected_rules_rust_root),
            None,
            false,
        )
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn new_for_test_with_source_charge_census(
        workspace: NormalizedAbsolutePath,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
        known_shas_root: Option<NormalizedAbsolutePath>,
    ) -> Result<Arc<Self>, SourceServiceError> {
        Self::new_for_test_with_offline_archive(
            workspace,
            work_deadline,
            cleanup_deadline,
            limits,
            known_shas_root,
            None,
            true,
        )
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn new_for_test_with_offline_archive(
        workspace: NormalizedAbsolutePath,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
        known_shas_root: Option<NormalizedAbsolutePath>,
        offline_archive: Option<Arc<super::OfflineArchiveBacking>>,
        source_charge_census: bool,
    ) -> Result<Arc<Self>, SourceServiceError> {
        Self::new_for_test_with_offline_archives(
            workspace,
            work_deadline,
            cleanup_deadline,
            limits,
            known_shas_root,
            [offline_archive, None],
            source_charge_census,
        )
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn new_for_test_with_offline_archives(
        workspace: NormalizedAbsolutePath,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
        known_shas_root: Option<NormalizedAbsolutePath>,
        offline_archives: [Option<Arc<super::OfflineArchiveBacking>>; 2],
        source_charge_census: bool,
    ) -> Result<Arc<Self>, SourceServiceError> {
        if !super::offline_archive::cohort_has_unique_urls(&offline_archives) {
            return Err(SourceServiceError::InvalidLimits);
        }
        let mut scope = Self::new(workspace, work_deadline, cleanup_deadline, limits)?;
        // Construction owns the sole Arc; no setter survives publication.
        let owned = Arc::get_mut(&mut scope).expect("new source scope is uniquely owned");
        owned.known_shas_root = known_shas_root;
        owned.offline_archives = offline_archives;
        if source_charge_census {
            owned
                .state
                .get_mut()
                .unwrap_or_else(|e| e.into_inner())
                .census = Some(Default::default());
        }
        Ok(scope)
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn offline_archive_path_for_test(
        &self,
        original_url: &str,
        integrity: &[u8; 32],
    ) -> Option<&NormalizedAbsolutePath> {
        super::offline_archive::cohort_path(&self.offline_archives, original_url, integrity)
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn known_shas_allowance_for_test(
        &self,
    ) -> Option<(&NormalizedAbsolutePath, usize)> {
        self.known_shas_root
            .as_ref()
            .map(|root| (root, super::regular_file::KNOWN_SHAS_OBJECT_BYTES))
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn known_shas_object_bytes_for_test(
        &self,
        demand: &slug_workspace_v2::PathObservationDemand,
    ) -> Option<usize> {
        use slug_workspace_v2::PathObservationNamespace;
        use slug_workspace_v2::PathObservationOperation;
        let (root, maximum) = self.known_shas_allowance_for_test()?;
        (demand.namespace() == PathObservationNamespace::Host
            && demand.operation() == PathObservationOperation::FileBytes
            && demand
                .path()
                .as_path()
                .strip_prefix(root.as_path())
                .is_ok_and(|relative| {
                    relative == std::path::Path::new("rust/private/known_shas.bzl")
                }))
        .then_some(maximum)
    }
    pub(in crate::runtime) fn workspace(&self) -> &NormalizedAbsolutePath {
        &self.workspace
    }
    pub(in crate::runtime) fn work_deadline(&self) -> Instant {
        self.work_deadline
    }
    pub(in crate::runtime) fn cleanup_deadline(&self) -> Instant {
        self.cleanup_deadline
    }
    pub(in crate::runtime) fn limits(&self) -> SourceServiceLimits {
        self.limits
    }
    fn state(&self) -> std::sync::MutexGuard<'_, ScopeState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    #[cfg_attr(all(test, target_os = "linux"), track_caller)]
    fn check_locked(
        &self,
        state: &mut ScopeState,
        allow_closed: bool,
    ) -> Result<(), SourceServiceError> {
        self.check_locked_at(
            state,
            allow_closed,
            SourceTimeoutSite::GenericScope,
            SourceTimeoutPhase::ScopeCheck,
        )
    }
    #[cfg_attr(all(test, target_os = "linux"), track_caller)]
    fn check_locked_at(
        &self,
        state: &mut ScopeState,
        allow_closed: bool,
        site: SourceTimeoutSite,
        phase: SourceTimeoutPhase,
    ) -> Result<(), SourceServiceError> {
        if let Some(error) = state.first {
            #[cfg(all(test, target_os = "linux"))]
            if error == SourceServiceError::WorkDeadline {
                state.timeout.sticky();
            }
            return Err(error);
        }
        let error = if *self.cancelled.borrow() {
            Some(SourceServiceError::Cancelled)
        } else if Instant::now() >= self.work_deadline {
            Some(SourceServiceError::WorkDeadline)
        } else if state.closed && !allow_closed {
            Some(SourceServiceError::AcquisitionClosed)
        } else {
            None
        };
        if let Some(error) = error {
            let origin = SourceTimeoutOrigin::common(site, phase);
            #[cfg(all(test, target_os = "linux"))]
            let origin = {
                let mut origin = origin;
                if error == SourceServiceError::WorkDeadline
                    && state.timeout.is_armed()
                    && site == SourceTimeoutSite::GenericScope
                    && phase == SourceTimeoutPhase::ScopeCheck
                {
                    let caller = std::panic::Location::caller();
                    origin.caller = Some(super::timeout_provenance::SourceTimeoutCaller::project(
                        caller.file(),
                        caller.line(),
                    ));
                }
                origin
            };
            let error = Self::poison_locked(state, error, origin);
            self.cancelled.send_replace(true);
            Err(error)
        } else {
            Ok(())
        }
    }
    #[cfg_attr(all(test, target_os = "linux"), track_caller)]
    pub(in crate::runtime) fn check(&self) -> Result<(), SourceServiceError> {
        self.check_locked(&mut self.state(), false)
    }
    pub(super) fn check_network(&self) -> Result<(), SourceServiceError> {
        self.check()?;
        if self.limits.allow_network {
            Ok(())
        } else {
            Err(self.poison(SourceServiceError::NetworkDisabled))
        }
    }
    pub(in crate::runtime) fn poison(&self, error: SourceServiceError) -> SourceServiceError {
        self.poison_at(error, SourceTimeoutOrigin::unattributed())
    }
    fn poison_locked(
        state: &mut ScopeState,
        error: SourceServiceError,
        _origin: SourceTimeoutOrigin,
    ) -> SourceServiceError {
        if let Some(first) = state.first {
            #[cfg(all(test, target_os = "linux"))]
            if first == SourceServiceError::WorkDeadline {
                state.timeout.sticky();
            }
            return first;
        }
        state.first = Some(error);
        #[cfg(all(test, target_os = "linux"))]
        if error == SourceServiceError::WorkDeadline {
            state.timeout.created(_origin);
        }
        error
    }
    fn poison_at(
        &self,
        error: SourceServiceError,
        origin: SourceTimeoutOrigin,
    ) -> SourceServiceError {
        let first = {
            let mut state = self.state();
            #[cfg(all(test, target_os = "linux"))]
            let created = state.first.is_none();
            let first = Self::poison_locked(&mut state, error, origin);
            #[cfg(all(test, target_os = "linux"))]
            if created && error == SourceServiceError::BufferLimit {
                let origin = SourceBufferOrigin::unknown(
                    state.counters.live_buffer_bytes,
                    self.limits.live_buffer_bytes,
                );
                state.buffer.created(origin);
            }
            first
        };
        self.cancelled.send_replace(true);
        first
    }
    pub(in crate::runtime) fn poison_buffer_limit_at(
        &self,
        _site: SourceBufferSite,
        _requested: usize,
        _bound: usize,
        _unit: SourceBufferUnit,
    ) -> SourceServiceError {
        let first = {
            let mut state = self.state();
            #[cfg(all(test, target_os = "linux"))]
            let created = state.first.is_none();
            let first = Self::poison_locked(
                &mut state,
                SourceServiceError::BufferLimit,
                SourceTimeoutOrigin::unattributed(),
            );
            #[cfg(all(test, target_os = "linux"))]
            if created {
                let origin = SourceBufferOrigin {
                    site: _site,
                    kind: SourceBufferKind::ExplicitBound,
                    unit: _unit,
                    requested: Some(_requested),
                    bound: Some(_bound),
                    live_bytes: state.counters.live_buffer_bytes,
                    bank_ceiling_bytes: self.limits.live_buffer_bytes,
                };
                state.buffer.created(origin);
            }
            first
        };
        self.cancelled.send_replace(true);
        first
    }
    pub(in crate::runtime) fn cancel(&self) {
        self.poison(SourceServiceError::Cancelled);
        self.cancelled.send_replace(true);
    }
    pub(in crate::runtime) fn snapshot(&self) -> SourceServiceCounters {
        self.state().counters
    }
    #[cfg(test)]
    pub(in crate::runtime::dice) fn first_error_for_test(&self) -> Option<SourceServiceError> {
        self.state().first
    }
    pub(in crate::runtime) fn charge(
        &self,
        counter: SourceCounter,
        amount: u64,
    ) -> Result<(), SourceServiceError> {
        #[cfg(all(test, target_os = "linux"))]
        if counter == SourceCounter::SourceAttempts {
            let (result, record) = self.charge_source_with_record_for_test(
                amount,
                super::SourceChargeDomain::Unassociated,
            );
            if let Some(record) = record {
                self.report_source_charge_for_test(
                    record,
                    super::SourceChargePhase::Unassociated,
                    super::SourceChargeIdentity::Anonymous,
                );
            }
            return result;
        }
        let mut state = self.state();
        self.charge_locked(&mut state, counter, amount)
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(super) fn charge_source_with_origin_record_for_test(
        &self,
        amount: u64,
        domain: super::SourceChargeDomain,
    ) -> (
        Result<(), SourceServiceError>,
        bool,
        Option<super::SourceChargeRecord>,
    ) {
        let mut state = self.state();
        let unpoisoned = state.first.is_none();
        let before = state.counters.source_attempts;
        let result = self.charge_locked(&mut state, SourceCounter::SourceAttempts, amount);
        let created =
            unpoisoned && result == Err(SourceServiceError::Limit(SourceCounter::SourceAttempts));
        let after = state.counters.source_attempts;
        let record = state
            .census
            .as_mut()
            .and_then(|c| c.record(amount, domain, before, after, result, created));
        (result, created, record)
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn charge_source_with_record_for_test(
        &self,
        amount: u64,
        domain: super::SourceChargeDomain,
    ) -> (
        Result<(), SourceServiceError>,
        Option<super::SourceChargeRecord>,
    ) {
        let (result, _, record) = self.charge_source_with_origin_record_for_test(amount, domain);
        (result, record)
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn charge_source_attempt_with_origin(
        &self,
    ) -> (Result<(), SourceServiceError>, bool) {
        let (result, created, record) = self
            .charge_source_with_origin_record_for_test(1, super::SourceChargeDomain::Unassociated);
        if let Some(record) = record {
            self.report_source_charge_for_test(
                record,
                super::SourceChargePhase::Unassociated,
                super::SourceChargeIdentity::Anonymous,
            );
        }
        (result, created)
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn report_source_charge_for_test(
        &self,
        record: super::SourceChargeRecord,
        phase: super::SourceChargePhase,
        identity: super::SourceChargeIdentity<'_>,
    ) {
        use std::io::Write;

        use super::source_charge_census::Row;
        use super::source_charge_census::encoded_len;
        if self.state().census.is_none() {
            return;
        }
        let row = Row::new(record, phase, identity);
        let Ok(bytes) = encoded_len(&row) else {
            if let Some(census) = self.state().census.as_mut() {
                census.encoding_failed(true);
            }
            return;
        };
        {
            let mut state = self.state();
            let Some(census) = state.census.as_mut() else {
                return;
            };
            if !census.reserve(&row, bytes) {
                return;
            }
        }
        // Only the existing stdout framing lock is held while borrowed bytes stream.
        let success = std::io::stdout()
            .lock()
            .write_fmt(format_args!("{row}"))
            .is_ok();
        if let Some(census) = self.state().census.as_mut() {
            census.finish(&row, bytes, success);
        }
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn report_source_charge_summary_for_test(&self) {
        use std::io::Write;
        let summary = {
            let state = self.state();
            let Some(census) = state.census.as_ref() else {
                return;
            };
            census
                .summary(self.limits.source_attempts, state.counters.source_attempts)
                .including_summary()
        };
        // A missing/partial summary is invalid evidence; never alter source errors.
        let success = summary.is_ok_and(|summary| {
            std::io::stdout()
                .lock()
                .write_fmt(format_args!("{summary}"))
                .is_ok()
        });
        if !success {
            if let Some(census) = self.state().census.as_mut() {
                census.encoding_failed(false);
            }
        }
    }
    fn charge_locked(
        &self,
        state: &mut ScopeState,
        counter: SourceCounter,
        amount: u64,
    ) -> Result<(), SourceServiceError> {
        self.check_locked(state, false)?;
        let old = *state.counters.slot(counter);
        let Some(next) = old.checked_add(amount) else {
            let e = SourceServiceError::Overflow(counter);
            state.first = Some(e);
            self.cancelled.send_replace(true);
            return Err(e);
        };
        if next > self.limits.maximum(counter) {
            let e = SourceServiceError::Limit(counter);
            state.first = Some(e);
            self.cancelled.send_replace(true);
            return Err(e);
        }
        *state.counters.slot(counter) = next;
        Ok(())
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(in crate::runtime) fn arm_registry_association_for_test(
        self: &Arc<Self>,
    ) -> Result<Arc<super::RegistryRequestAssociation>, SourceServiceError> {
        self.arm_registry_association_with_hook(|| {})
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(super) fn arm_registry_association_with_hook(
        self: &Arc<Self>,
        before_attachment: impl FnOnce(),
    ) -> Result<Arc<super::RegistryRequestAssociation>, SourceServiceError> {
        {
            let mut state = self.state();
            self.check_locked(&mut state, false)?;
            if state.counters.registry_attempts != 0 || self.registry_association.get().is_some() {
                return Err(SourceServiceError::ForeignScope);
            }
        }
        // Allocate/reserve outside the state lock. Revalidate before publication.
        let association = super::RegistryRequestAssociation::new(self)?;
        before_attachment(); // Deterministic private test boundary, never under a lock.
        let mut state = self.state();
        self.check_locked(&mut state, false)?;
        if state.counters.registry_attempts != 0 || self.registry_association.get().is_some() {
            drop(state);
            return Err(SourceServiceError::ForeignScope);
        }
        let attached = self.registry_association.set(Arc::downgrade(&association));
        if attached.is_ok() {
            state.timeout.arm();
            state.buffer.arm();
        }
        drop(state);
        attached.map_err(|_| SourceServiceError::ForeignScope)?;
        Ok(association)
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(super) fn disarm_timeout_for_test(
        &self,
        recorder: *const super::RegistryRequestAssociation,
    ) {
        let mut state = self.state();
        if self
            .registry_association
            .get()
            .is_some_and(|weak| weak.as_ptr() == recorder)
        {
            state.timeout.clear();
            state.buffer.clear();
        }
        // No recorder upgrade or association lock; guard drops before credit Drop.
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(super) fn registry_observation_for_test(
        &self,
    ) -> (
        SourceServiceCounters,
        SourceTimeoutSnapshot,
        SourceBufferSnapshot,
    ) {
        let state = self.state();
        (
            state.counters,
            state
                .timeout
                .snapshot(state.first == Some(SourceServiceError::WorkDeadline)),
            state
                .buffer
                .snapshot(state.first == Some(SourceServiceError::BufferLimit)),
        )
    }
    #[cfg(all(test, target_os = "linux"))]
    pub(super) fn charge_registry_request_for_test(
        &self,
        request: super::registry_association::RegistryRequestRef<'_>,
    ) -> Result<(), SourceServiceError> {
        let Some(association) = self
            .registry_association
            .get()
            .and_then(std::sync::Weak::upgrade)
        else {
            return self.charge(SourceCounter::RegistryAttempts, 1);
        };
        association.charge(request, || {
            let mut state = self.state();
            let before = state.counters.registry_attempts;
            let had_poison = state.first.is_some();
            let result = self.charge_locked(&mut state, SourceCounter::RegistryAttempts, 1);
            let created_limit = !had_poison
                && state.first == Some(SourceServiceError::Limit(SourceCounter::RegistryAttempts));
            super::registry_association::RegistryChargeObservation {
                before,
                after: state.counters.registry_attempts,
                created_limit,
                result,
            }
            // State guard drops before the possibly last association Arc/credit.
        })
    }
    pub(in crate::runtime) fn remaining(
        &self,
        counter: SourceCounter,
    ) -> Result<u64, SourceServiceError> {
        let mut state = self.state();
        self.check_locked(&mut state, false)?;
        Ok(self.limits.maximum(counter) - *state.counters.slot(counter))
    }
    pub(in crate::runtime) fn reserve_buffer(
        self: &Arc<Self>,
        bytes: usize,
    ) -> Result<SourceBufferReservation, SourceServiceError> {
        self.reserve_buffer_at(bytes, SourceBufferSite::Unattributed)
    }
    pub(in crate::runtime) fn reserve_buffer_at(
        self: &Arc<Self>,
        bytes: usize,
        _site: SourceBufferSite,
    ) -> Result<SourceBufferReservation, SourceServiceError> {
        let mut state = self.state();
        self.check_locked(&mut state, false)?;
        let Some(next) = state.counters.live_buffer_bytes.checked_add(bytes) else {
            state.first = Some(SourceServiceError::BufferLimit);
            #[cfg(all(test, target_os = "linux"))]
            self.record_buffer_reservation(
                &mut state,
                _site,
                bytes,
                SourceBufferKind::ReservationOverflow,
            );
            self.cancelled.send_replace(true);
            return Err(SourceServiceError::BufferLimit);
        };
        if next > self.limits.live_buffer_bytes {
            state.first = Some(SourceServiceError::BufferLimit);
            #[cfg(all(test, target_os = "linux"))]
            self.record_buffer_reservation(
                &mut state,
                _site,
                bytes,
                SourceBufferKind::ReservationCeiling,
            );
            self.cancelled.send_replace(true);
            return Err(SourceServiceError::BufferLimit);
        }
        state.counters.live_buffer_bytes = next;
        state.counters.peak_buffer_bytes = state.counters.peak_buffer_bytes.max(next);
        Ok(SourceBufferReservation {
            scope: Arc::clone(self),
            bytes,
        })
    }
    #[cfg(all(test, target_os = "linux"))]
    fn record_buffer_reservation(
        &self,
        state: &mut ScopeState,
        site: SourceBufferSite,
        requested: usize,
        kind: SourceBufferKind,
    ) {
        state.buffer.created(SourceBufferOrigin {
            site,
            kind,
            unit: SourceBufferUnit::Bytes,
            requested: Some(requested),
            bound: Some(self.limits.live_buffer_bytes),
            live_bytes: state.counters.live_buffer_bytes,
            bank_ceiling_bytes: self.limits.live_buffer_bytes,
        });
    }
    pub(in crate::runtime) async fn during<T, F>(
        &self,
        step: Duration,
        future: F,
    ) -> Result<T, SourceServiceError>
    where
        F: Future<Output = Result<T, SourceServiceError>>,
    {
        self.during_at(SourceTimeoutSite::GenericWait, step, future)
            .await
    }
    pub(in crate::runtime) async fn during_at<T, F>(
        &self,
        site: SourceTimeoutSite,
        step: Duration,
        future: F,
    ) -> Result<T, SourceServiceError>
    where
        F: Future<Output = Result<T, SourceServiceError>>,
    {
        let mut cancel = self.cancelled.subscribe();
        self.check_locked_at(
            &mut self.state(),
            false,
            site,
            SourceTimeoutPhase::DuringEntryCheck,
        )?;
        let chosen = SourceTimeoutDeadline::select(self.work_deadline, Instant::now(), step);
        let deadline = chosen.deadline;
        let result = tokio::select! { biased;
            _=cancel.changed()=>return Err(self.poison(SourceServiceError::Cancelled)),
            _=tokio::time::sleep_until(deadline.into())=>return Err(self.poison_at(
                SourceServiceError::WorkDeadline,
                chosen.origin(site, SourceTimeoutPhase::Timer, SourceTimeoutReturn::Deadline))),
            value=future=>value,
        };
        if Instant::now() >= deadline {
            let disposition = if result.is_err() {
                SourceTimeoutReturn::CompletedPrimary
            } else {
                SourceTimeoutReturn::Deadline
            };
            let expired = self.poison_at(
                SourceServiceError::WorkDeadline,
                chosen.origin(site, SourceTimeoutPhase::Postcheck, disposition),
            );
            return match result {
                Err(primary) => Err(primary),
                Ok(_) => Err(expired),
            };
        }
        let value = result.map_err(|error| self.poison(error))?;
        #[cfg(all(test, target_os = "linux"))]
        super::timeout_provenance::before_final_check_for_test();
        self.check_locked_at(
            &mut self.state(),
            false,
            site,
            SourceTimeoutPhase::DuringFinalCheck,
        )?;
        Ok(value)
    }
    pub(in crate::runtime) async fn during_cleanup<T, F>(
        &self,
        future: F,
    ) -> Result<T, SourceServiceError>
    where
        F: Future<Output = Result<T, SourceServiceError>>,
    {
        if Instant::now() >= self.cleanup_deadline {
            return Err(self.poison(SourceServiceError::CleanupDeadline));
        }
        let result = tokio::select! {biased;
            _=tokio::time::sleep_until(self.cleanup_deadline.into())=>Err(self.poison(SourceServiceError::CleanupDeadline)),
            value=future=>value,
        };
        if Instant::now() >= self.cleanup_deadline {
            let expired = self.poison(SourceServiceError::CleanupDeadline);
            // A synchronous Future::poll may cross the deadline before the
            // timer can be polled again. It cannot manufacture joined success.
            // Keep a completed primary refusal if cleanup also expired.
            return match result {
                Err(primary) => Err(primary),
                Ok(_) => Err(expired),
            };
        }
        result
    }
    pub(super) fn close_acquisition(&self) -> Result<(), SourceServiceError> {
        let mut state = self.state();
        self.check_locked(&mut state, false)?;
        state.closed = true;
        Ok(())
    }
    pub(super) fn check_closed_export(&self) -> Result<(), SourceServiceError> {
        let mut state = self.state();
        self.check_locked(&mut state, true)?;
        if !state.closed {
            return Err(SourceServiceError::AcquisitionClosed);
        }
        Ok(())
    }
}

pub(in crate::runtime) struct SourceBufferReservation {
    scope: Arc<SourceServiceScope>,
    bytes: usize,
}
#[cfg(all(test, target_os = "linux"))]
impl SourceBufferReservation {
    pub(super) fn same_scope(&self, scope: &Arc<SourceServiceScope>) -> bool {
        Arc::ptr_eq(&self.scope, scope)
    }
    pub(super) fn registry_observation(
        &self,
    ) -> (
        SourceServiceCounters,
        SourceTimeoutSnapshot,
        SourceBufferSnapshot,
    ) {
        self.scope.registry_observation_for_test()
    }
    pub(super) fn disarm_timeout(&self, recorder: *const super::RegistryRequestAssociation) {
        self.scope.disarm_timeout_for_test(recorder);
    }
}
impl Drop for SourceBufferReservation {
    fn drop(&mut self) {
        let mut state = self.scope.state();
        state.counters.live_buffer_bytes -= self.bytes;
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "source_attempt_origin_tests.rs"]
mod source_attempt_origin_tests;

#[cfg(all(test, target_os = "linux"))]
pub(super) fn census_layout_for_test() -> (usize, usize, usize, usize) {
    // Test-only mirrors of the immediately preceding field lists, not runtime owners.
    #[allow(dead_code)]
    struct PriorState {
        counters: SourceServiceCounters,
        first: Option<SourceServiceError>,
        closed: bool,
        timeout: SourceTimeoutState,
        buffer: SourceBufferState,
    }
    #[allow(dead_code)]
    struct PriorScope {
        workspace: NormalizedAbsolutePath,
        work_deadline: Instant,
        cleanup_deadline: Instant,
        limits: SourceServiceLimits,
        cancelled: tokio::sync::watch::Sender<bool>,
        state: Mutex<PriorState>,
        registry_association:
            std::sync::OnceLock<std::sync::Weak<super::RegistryRequestAssociation>>,
        known_shas_root: Option<NormalizedAbsolutePath>,
    }
    (
        std::mem::size_of::<PriorState>(),
        std::mem::size_of::<ScopeState>(),
        std::mem::size_of::<PriorScope>(),
        std::mem::size_of::<SourceServiceScope>(),
    )
}
