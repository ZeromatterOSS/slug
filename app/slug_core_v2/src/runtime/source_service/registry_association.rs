//! Optional exact identities at the natural charge owner; never source authority.
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use slug_bzlmod_v2::RegistryFileUrl;

use super::SourceBufferReservation;
use super::SourceCounter;
use super::SourceServiceError;
use super::SourceServiceLimits;
use super::SourceServiceScope;
use super::timeout_provenance::SourceTimeoutSnapshot;
use super::timeout_provenance::SourceTimeoutState;

#[path = "registry_association/tests.rs"]
mod tests;

const ROWS: usize = SourceServiceLimits::MAX_REGISTRY_ATTEMPTS as usize;
const IDENTITY_BYTES: usize = 8192;
const ROW_BYTES: usize = IDENTITY_BYTES * 2;

pub(super) enum RegistryRequestRef<'a> {
    Url(&'a RegistryFileUrl),
    Local {
        url: &'a RegistryFileUrl,
        path: &'a Path,
    },
}
impl RegistryRequestRef<'_> {
    fn bytes(&self) -> (&[u8], Option<&[u8]>) {
        match self {
            Self::Url(url) => (url.as_str().as_bytes(), None),
            Self::Local { url, path } => {
                (url.as_str().as_bytes(), Some(path.as_os_str().as_bytes()))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) enum RegistryLimitRelation {
    #[default]
    Unknown,
    ExistingRow {
        row_ordinal: u64,
        prior_admitted_count: u64,
    },
    NewDistinctUnretained {
        unique_ordinal: u64,
    },
}
impl RegistryLimitRelation {
    pub(in crate::runtime) fn code(self) -> u64 {
        match self {
            Self::Unknown => 0,
            Self::ExistingRow { .. } => 1,
            Self::NewDistinctUnretained { .. } => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::runtime) struct RegistryAssociationSnapshot {
    pub(in crate::runtime) complete: bool,
    pub(in crate::runtime) attempted_entries: u64,
    pub(in crate::runtime) admitted_entries: u64,
    pub(in crate::runtime) scope_admitted_entries: u64,
    pub(in crate::runtime) refused_entries: u64,
    pub(in crate::runtime) sticky_refused_entries: u64,
    pub(in crate::runtime) admitted_distinct: u64,
    pub(in crate::runtime) admitted_repeated: u64,
    pub(in crate::runtime) retained_rows: u64,
    pub(in crate::runtime) first_limit_call_ordinal: Option<u64>,
    pub(in crate::runtime) first_limit_relation: RegistryLimitRelation,
    pub(in crate::runtime) charged_bytes: usize,
    pub(in crate::runtime) timeout: SourceTimeoutSnapshot,
}
impl RegistryAssociationSnapshot {
    fn new(charged_bytes: usize) -> Self {
        Self {
            complete: true,
            attempted_entries: 0,
            admitted_entries: 0,
            scope_admitted_entries: 0,
            refused_entries: 0,
            sticky_refused_entries: 0,
            admitted_distinct: 0,
            admitted_repeated: 0,
            retained_rows: 0,
            first_limit_call_ordinal: None,
            first_limit_relation: RegistryLimitRelation::Unknown,
            charged_bytes,
            timeout: SourceTimeoutSnapshot::default(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Row {
    url_length: usize,
    local_length: Option<usize>,
    admissions: u64,
}
struct AssociationState {
    slab: Vec<u8>,
    rows: [Row; ROWS],
    snapshot: RegistryAssociationSnapshot,
}
pub(in crate::runtime) struct RegistryRequestAssociation {
    state: Mutex<AssociationState>,
    // Storage (including rows/lock) drops before the Arc<Scope>-holding credit.
    credit: SourceBufferReservation,
}
pub(super) struct RegistryChargeObservation {
    pub(super) before: u64,
    pub(super) after: u64,
    pub(super) created_limit: bool,
    pub(super) result: Result<(), SourceServiceError>,
}
impl RegistryRequestAssociation {
    pub(super) fn new(scope: &Arc<SourceServiceScope>) -> Result<Arc<Self>, SourceServiceError> {
        let slab_bytes = usize::try_from(scope.limits().registry_attempts)
            .ok()
            .and_then(|rows| rows.checked_mul(ROW_BYTES))
            .ok_or(SourceServiceError::BufferLimit)?;
        // Includes fixed rows/lock/Arc headers and construction-header overlap.
        let charged = slab_bytes
            .checked_add(
                std::mem::size_of::<Self>()
                    .checked_mul(2)
                    .ok_or(SourceServiceError::BufferLimit)?,
            )
            .and_then(|n| n.checked_add(std::mem::size_of::<SourceTimeoutState>()))
            .and_then(|n| n.checked_add(64))
            .ok_or(SourceServiceError::BufferLimit)?;
        let credit = scope.reserve_buffer(charged)?;
        let mut slab = Vec::new();
        slab.try_reserve_exact(slab_bytes)
            .map_err(|_| scope.poison(SourceServiceError::Allocation))?;
        if slab.capacity() > slab_bytes {
            return Err(scope.poison(SourceServiceError::Allocation));
        }
        slab.resize(slab_bytes, 0);
        Ok(Arc::new(Self {
            state: Mutex::new(AssociationState {
                slab,
                rows: [Row::default(); ROWS],
                snapshot: RegistryAssociationSnapshot::new(charged),
            }),
            credit,
        }))
    }
    pub(in crate::runtime) fn same_scope(&self, scope: &Arc<SourceServiceScope>) -> bool {
        self.credit.same_scope(scope)
    }
    pub(in crate::runtime) fn snapshot(&self) -> RegistryAssociationSnapshot {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut snapshot = state.snapshot;
        let (counters, timeout) = self.credit.registry_observation();
        snapshot.scope_admitted_entries = counters.registry_attempts;
        snapshot.timeout = timeout;
        snapshot.complete &= snapshot.scope_admitted_entries == snapshot.admitted_entries;
        if !snapshot.complete {
            snapshot.first_limit_relation = RegistryLimitRelation::Unknown;
        }
        snapshot
    }
    pub(super) fn charge(
        &self,
        request: RegistryRequestRef<'_>,
        charge: impl FnOnce() -> RegistryChargeObservation,
    ) -> Result<(), SourceServiceError> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let observation = charge(); // Original scope state check+charge, no IO/await.
        let (url, local) = request.bytes();
        if observation.before != state.snapshot.admitted_entries {
            state.snapshot.complete = false; // Generic or unavailable charge gap.
        }
        let bounded =
            url.len() <= IDENTITY_BYTES && local.is_none_or(|p| p.len() <= IDENTITY_BYTES);
        if !bounded {
            state.snapshot.complete = false;
        }
        let call = bump(&mut state.snapshot.attempted_entries);
        if call.is_none() {
            state.snapshot.complete = false;
        }
        let found = bounded
            .then(|| {
                (0..state.snapshot.retained_rows as usize).find(|&index| {
                    let row = state.rows[index];
                    let base = index * ROW_BYTES;
                    row.local_length == local.map(<[u8]>::len)
                        && row.url_length == url.len()
                        && &state.slab[base..base + row.url_length] == url
                        && local.is_none_or(|p| {
                            &state.slab[base + IDENTITY_BYTES..base + IDENTITY_BYTES + p.len()] == p
                        })
                })
            })
            .flatten();
        let relation = if !state.snapshot.complete {
            RegistryLimitRelation::Unknown
        } else if let Some(index) = found {
            RegistryLimitRelation::ExistingRow {
                row_ordinal: index as u64 + 1,
                prior_admitted_count: state.rows[index].admissions,
            }
        } else if let Some(unique_ordinal) = state.snapshot.admitted_distinct.checked_add(1) {
            RegistryLimitRelation::NewDistinctUnretained { unique_ordinal }
        } else {
            state.snapshot.complete = false;
            RegistryLimitRelation::Unknown
        };
        state.snapshot.scope_admitted_entries = observation.after;
        if observation.result.is_ok() {
            if bump(&mut state.snapshot.admitted_entries).is_none() {
                state.snapshot.complete = false;
            }
            if let Some(index) = found {
                if bump(&mut state.rows[index].admissions).is_none()
                    || bump(&mut state.snapshot.admitted_repeated).is_none()
                {
                    state.snapshot.complete = false;
                }
            } else if bounded
                && state.snapshot.retained_rows < (state.slab.len() / ROW_BYTES) as u64
            {
                let index = state.snapshot.retained_rows as usize;
                let base = index * ROW_BYTES;
                state.slab[base..base + url.len()].copy_from_slice(url);
                if let Some(path) = local {
                    state.slab[base + IDENTITY_BYTES..base + IDENTITY_BYTES + path.len()]
                        .copy_from_slice(path);
                }
                state.rows[index] = Row {
                    url_length: url.len(),
                    local_length: local.map(<[u8]>::len),
                    admissions: 1,
                };
                state.snapshot.retained_rows += 1;
                if bump(&mut state.snapshot.admitted_distinct).is_none() {
                    state.snapshot.complete = false;
                }
            } else {
                state.snapshot.complete = false;
            }
        } else {
            if bump(&mut state.snapshot.refused_entries).is_none() {
                state.snapshot.complete = false;
            }
            if observation.created_limit {
                state.snapshot.first_limit_call_ordinal = call;
                state.snapshot.first_limit_relation = relation;
            } else if observation.result
                == Err(SourceServiceError::Limit(SourceCounter::RegistryAttempts))
                && bump(&mut state.snapshot.sticky_refused_entries).is_none()
            {
                state.snapshot.complete = false;
            }
        }
        observation.result
    }
}
fn bump(value: &mut u64) -> Option<u64> {
    let next = value.checked_add(1)?;
    *value = next;
    Some(next)
}

impl Drop for RegistryRequestAssociation {
    fn drop(&mut self) {
        // Exact pointer guard keeps a failed competing arm from clearing a winner.
        // Scope-only lock is released before state/slab and credit fields drop.
        self.credit.disarm_timeout(self as *const Self);
    }
}
