//! Bounded recovery after one genuine fresh-payload budget refusal. This
//! cursor grants no IO or content authority; its leases require full FD proof.
use super::*;

const MAX_ROWS: usize = 128;
const MAX_COMPARISONS: usize = 4;
const MAX_BYTES: usize = 512 * 1024 * 1024;

pub(in crate::runtime) enum NativeBodyAdmission {
    New(PendingNativeBody),
    Refused(NativeBodyRefusal),
}

/// Closed, noncloneable witness of the precise payload reservation refusal.
pub(in crate::runtime) struct NativeBodyRefusal {
    length: usize,
    origin: Arc<Origin>,
}

pub(in crate::runtime) struct NativeBodyRecovery<'a> {
    candidates: &'a NativeBodyCandidates,
    length: usize,
    pending_end: usize,
    committed_end: usize,
    pending_index: usize,
    committed_index: usize,
    rows: usize,
    comparisons: usize,
    compared_bytes: usize,
    _credit: ImageDescriptorReservation,
}

fn control_bytes() -> Result<usize, PhysicalRootError> {
    // The cursor/token, in-flight and returned lease slots, and a comparison
    // offset/equality closure remain paid before search. RootIo's existing
    // snapshot scratch independently covers buffers, frames and syscall state.
    add(
        add(
            std::mem::size_of::<NativeBodyRecovery<'_>>(),
            std::mem::size_of::<NativeBodyRefusal>(),
        )?,
        add(
            2 * std::mem::size_of::<Result<Option<NativeBodyLease>, PhysicalRootError>>(),
            std::mem::size_of::<(usize, bool)>(),
        )?,
    )
}

impl PendingNativeBody {
    /// Only exact retained block allocation pressure mints a recovery witness.
    pub(in crate::runtime) fn admit_packed_arc(
        &mut self,
        bytes: usize,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        self.origin.check(active)?;
        match self.credit.try_grow(bytes) {
            Ok(()) => Ok(()),
            Err(slug_workspace_v2::ImageDescriptorBudgetError::ResourceLimit) => {
                self.payload_refused = true;
                Err(PhysicalRootError::Resource)
            }
            Err(_) => Err(PhysicalRootError::Resource),
        }
    }
    pub(in crate::runtime) fn take_payload_refusal(&mut self) -> Option<NativeBodyRefusal> {
        std::mem::take(&mut self.payload_refused).then(|| NativeBodyRefusal {
            length: self.length,
            origin: self.origin.clone(),
        })
    }
}

impl NativeBodyCandidates {
    pub(in crate::runtime) fn reserve_new_for_capture(
        &mut self,
        length: usize,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeBodyAdmission, PhysicalRootError> {
        self.ledger.origin.check(active)?;
        // Map admission and every checked layout/control computation are fatal;
        // a caller must never mistake arbitrary Resource for payload pressure.
        self.storage.ensure(1)?;
        let bytes = add(
            body_bytes(length)?,
            std::mem::size_of::<PendingNativeBody>(),
        )?;
        match self.ledger.origin.resources.try_reserve_body_bytes(bytes)? {
            Some(credit) => Ok(NativeBodyAdmission::New(PendingNativeBody {
                length,
                origin: self.ledger.origin.clone(),
                credit,
                payload_refused: false,
            })),
            None => Ok(NativeBodyAdmission::Refused(NativeBodyRefusal {
                length,
                origin: self.ledger.origin.clone(),
            })),
        }
    }

    pub(in crate::runtime) fn recovery(
        &self,
        refusal: NativeBodyRefusal,
        active: &dyn Fn() -> bool,
    ) -> Result<Option<NativeBodyRecovery<'_>>, PhysicalRootError> {
        self.ledger.origin.check(active)?;
        if !Arc::ptr_eq(&refusal.origin, &self.ledger.origin) {
            return Err(PhysicalRootError::Reservation);
        }
        if refusal.length > MAX_BYTES {
            return Ok(None);
        }
        let Some(credit) = self
            .ledger
            .origin
            .resources
            .try_reserve_body_bytes(control_bytes()?)?
        else {
            return Ok(None);
        };
        let pending_end = self.storage.bodies.len();
        let committed_end = self
            .ledger
            .storage
            .lock()
            .map_err(|_| PhysicalRootError::Resource)?
            .bodies
            .len();
        self.ledger.origin.check(active)?;
        if pending_end == 0 && committed_end == 0 {
            return Ok(None);
        }
        Ok(Some(NativeBodyRecovery {
            candidates: self,
            length: refusal.length,
            pending_end,
            committed_end,
            pending_index: 0,
            committed_index: 0,
            rows: 0,
            comparisons: 0,
            compared_bytes: 0,
            _credit: credit,
        }))
    }
}

impl NativeBodyRecovery<'_> {
    /// Returns only exact-length leases. Each physical map row consumes the
    /// shared row budget; each returned lease prepays a full comparison BEFORE
    /// caller IO, even when it mismatches early. Later appends stay invisible.
    pub(in crate::runtime) fn next(
        &mut self,
        active: &dyn Fn() -> bool,
    ) -> Result<Option<NativeBodyLease>, PhysicalRootError> {
        loop {
            self.candidates.ledger.origin.check(active)?;
            if self.rows == MAX_ROWS || self.comparisons == MAX_COMPARISONS {
                return Ok(None);
            }
            let Some(bytes) = self
                .compared_bytes
                .checked_add(self.length)
                .filter(|n| *n <= MAX_BYTES)
            else {
                return Ok(None);
            };
            let lease = if self.pending_index < self.pending_end {
                let (_, lease) = self
                    .candidates
                    .storage
                    .bodies
                    .get_index(self.pending_index)
                    .ok_or(PhysicalRootError::Reservation)?;
                self.pending_index += 1;
                (lease.body().len() == self.length).then(|| lease.clone())
            } else if self.committed_index < self.committed_end {
                // No Source/active callback or IO executes while this guard
                // exists. SmallMap is append-only, so captured indices persist.
                let selected = {
                    let storage = self
                        .candidates
                        .ledger
                        .storage
                        .lock()
                        .map_err(|_| PhysicalRootError::Resource)?;
                    let (_, lease) = storage
                        .bodies
                        .get_index(self.committed_index)
                        .ok_or(PhysicalRootError::Reservation)?;
                    (lease.body().len() == self.length).then(|| lease.clone())
                };
                self.committed_index += 1;
                selected
            } else {
                return Ok(None);
            };
            self.rows += 1;
            self.candidates.ledger.origin.check(active)?;
            if let Some(lease) = lease {
                self.comparisons += 1;
                self.compared_bytes = bytes;
                return Ok(Some(lease));
            }
        }
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
