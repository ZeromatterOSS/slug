use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use slug_workspace_v2::NormalizedAbsolutePath;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SourceCounter {
    RegistryAttempts,
    RegistryBytes,
    SourceAttempts,
    TransferBytes,
    DecodedBytes,
    WrittenBytes,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::runtime) struct SourceServiceLimits {
    pub(in crate::runtime) registry_object_bytes: u64,
    pub(in crate::runtime) registry_attempts: u64,
    pub(in crate::runtime) registry_admitted_bytes: u64,
    pub(in crate::runtime) source_attempts: u64,
    pub(in crate::runtime) transferred_bytes: u64,
    pub(in crate::runtime) decoded_bytes: u64,
    pub(in crate::runtime) written_bytes: u64,
    pub(in crate::runtime) live_buffer_bytes: usize,
}
impl Default for SourceServiceLimits {
    fn default() -> Self {
        Self {
            registry_object_bytes: 1 << 20,
            registry_attempts: 128,
            registry_admitted_bytes: 128 << 20,
            source_attempts: 64,
            transferred_bytes: 2 << 30,
            decoded_bytes: 2 << 30,
            written_bytes: 2 << 30,
            live_buffer_bytes: 16 << 20,
        }
    }
}
impl SourceServiceLimits {
    fn valid(self) -> bool {
        let max = Self::default();
        [
            (self.registry_object_bytes, max.registry_object_bytes),
            (self.registry_attempts, max.registry_attempts),
            (self.registry_admitted_bytes, max.registry_admitted_bytes),
            (self.source_attempts, max.source_attempts),
            (self.transferred_bytes, max.transferred_bytes),
            (self.decoded_bytes, max.decoded_bytes),
            (self.written_bytes, max.written_bytes),
        ]
        .into_iter()
        .all(|(v, m)| v > 0 && v <= m)
            && self.live_buffer_bytes > 0
            && self.live_buffer_bytes <= max.live_buffer_bytes
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
    fn check_locked(
        &self,
        state: &mut ScopeState,
        allow_closed: bool,
    ) -> Result<(), SourceServiceError> {
        if let Some(error) = state.first {
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
            state.first = Some(error);
            self.cancelled.send_replace(true);
            Err(error)
        } else {
            Ok(())
        }
    }
    pub(in crate::runtime) fn check(&self) -> Result<(), SourceServiceError> {
        self.check_locked(&mut self.state(), false)
    }
    pub(in crate::runtime) fn poison(&self, error: SourceServiceError) -> SourceServiceError {
        let first = {
            let mut state = self.state();
            *state.first.get_or_insert(error)
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
        let mut state = self.state();
        self.charge_locked(&mut state, counter, amount)
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
        drop(state);
        attached.map_err(|_| SourceServiceError::ForeignScope)?;
        Ok(association)
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
        let mut state = self.state();
        self.check_locked(&mut state, false)?;
        let Some(next) = state.counters.live_buffer_bytes.checked_add(bytes) else {
            state.first = Some(SourceServiceError::BufferLimit);
            self.cancelled.send_replace(true);
            return Err(SourceServiceError::BufferLimit);
        };
        if next > self.limits.live_buffer_bytes {
            state.first = Some(SourceServiceError::BufferLimit);
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
    pub(in crate::runtime) async fn during<T, F>(
        &self,
        step: Duration,
        future: F,
    ) -> Result<T, SourceServiceError>
    where
        F: Future<Output = Result<T, SourceServiceError>>,
    {
        let mut cancel = self.cancelled.subscribe();
        self.check()?;
        let deadline = self.work_deadline.min(
            Instant::now()
                .checked_add(step)
                .unwrap_or(self.work_deadline),
        );
        let result = tokio::select! { biased;
            _=cancel.changed()=>return Err(self.poison(SourceServiceError::Cancelled)),
            _=tokio::time::sleep_until(deadline.into())=>return Err(self.poison(SourceServiceError::WorkDeadline)),
            value=future=>value,
        };
        if Instant::now() >= deadline {
            let expired = self.poison(SourceServiceError::WorkDeadline);
            return match result {
                Err(primary) => Err(primary),
                Ok(_) => Err(expired),
            };
        }
        let value = result.map_err(|error| self.poison(error))?;
        self.check()?;
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
    pub(super) fn registry_attempts(&self) -> u64 {
        self.scope.snapshot().registry_attempts
    }
}
impl Drop for SourceBufferReservation {
    fn drop(&mut self) {
        let mut state = self.scope.state();
        state.counters.live_buffer_bytes -= self.bytes;
    }
}
