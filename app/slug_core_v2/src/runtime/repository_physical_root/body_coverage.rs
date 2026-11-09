//! Native allocation coverage only. Pointer membership never authenticates
//! semantic state, request provenance, file contents, or filesystem authority.
use std::alloc::Layout;
use std::fmt;
use std::sync::Mutex;

use slug_workspace_v2::BudgetedImageBlock;
use slug_workspace_v2::ImageDescriptorReservation;
use starlark_map::StarlarkHashValue;
use starlark_map::small_map::SmallMap;

use super::*;

mod recovery;
pub(in crate::runtime) use recovery::NativeBodyAdmission;
pub(in crate::runtime) use recovery::NativeBodyRefusal;

type BodyKey = (usize, usize);
type Visitor<'a> = dyn FnMut(&Arc<[u8]>) -> Result<(), PhysicalRootError> + 'a;

fn key(body: &Arc<[u8]>) -> BodyKey {
    (body.as_ptr() as usize, body.len())
}

#[derive(Clone)]
pub(in crate::runtime) struct NativeBodyLease(BudgetedImageBlock<Arc<[u8]>>);
impl NativeBodyLease {
    pub(in crate::runtime) fn body(&self) -> &Arc<[u8]> {
        self.0.get()
    }
}
impl fmt::Debug for NativeBodyLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeBodyLease")
            .field("length", &self.body().len())
            .finish_non_exhaustive()
    }
}

struct Origin {
    resources: Arc<RepositoryRootResources>,
    scope: Arc<SourceServiceScope>,
    reservation: Arc<RepositoryWorkspaceReservation>,
    // The associated data owners precede their control reservation.
    _credit: ImageDescriptorReservation,
}
impl Origin {
    fn check(&self, active: &dyn Fn() -> bool) -> Result<(), PhysicalRootError> {
        check(&self.scope, active)
    }
}

/// A single root-local, flat, append-only collection. Never enters DICE.
pub(in crate::runtime) struct NativeBodyLedger {
    storage: Mutex<BodyMap>,
    origin: Arc<Origin>,
    _credit: ImageDescriptorReservation,
}

/// Unknown incoming/capture allocations. Keep this alive until all corresponding
/// raw semantic owners have retired, including on failed completion/commit.
pub(in crate::runtime) struct NativeBodyCandidates {
    storage: BodyMap,
    ledger: Arc<NativeBodyLedger>,
    _credit: ImageDescriptorReservation,
}

/// Filtered final-data coverage, deliberately without a ledger/history edge.
pub(in crate::runtime) struct NativeBodySet {
    storage: BodyMap,
    origin: Arc<Origin>,
    _credit: ImageDescriptorReservation,
}

/// Admission for a fresh allocation. Allocate its actual Arc only AFTER this
/// succeeds. On a failed finish, the unpublished body drops before this credit.
pub(in crate::runtime) struct PendingNativeBody {
    length: usize,
    origin: Arc<Origin>,
    credit: ImageDescriptorReservation,
}

struct BodyMap {
    bodies: SmallMap<BodyKey, NativeBodyLease>,
    credit: ImageDescriptorReservation,
}
impl BodyMap {
    fn new(resources: &RepositoryRootResources) -> Result<Self, PhysicalRootError> {
        Ok(Self {
            bodies: SmallMap::new(),
            credit: resources.reserve_body_bytes(std::mem::size_of::<Self>())?,
        })
    }
    fn ensure(&mut self, additional: usize) -> Result<(), PhysicalRootError> {
        let required = add(self.bodies.len(), additional)?;
        if required <= self.bodies.capacity() {
            return Ok(());
        }
        let next = required.max(4).max(
            self.bodies
                .capacity()
                .checked_mul(2)
                .ok_or(PhysicalRootError::Resource)?,
        );
        let bound = map_bound(next)?;
        // Retain all prior table credits while Vec2/index resize. Geometric
        // reservations sum to O(final capacity), with no version-table copies.
        self.credit
            .try_grow(bound)
            .map_err(|_| PhysicalRootError::Resource)?;
        self.bodies.reserve(next - self.bodies.len());
        if self.bodies.capacity() > next
            || self
                .bodies
                .retained_storage_bytes_upper_bound()
                .ok_or(PhysicalRootError::Resource)?
                > bound
        {
            return Err(PhysicalRootError::Resource);
        }
        Ok(())
    }
    fn insert(&mut self, lease: NativeBodyLease) -> Result<(), PhysicalRootError> {
        let k = key(lease.body());
        if !self.bodies.contains_key(&k) {
            self.ensure(1)?;
            self.bodies.insert(k, lease);
        }
        Ok(())
    }
}

fn map_bound(capacity: usize) -> Result<usize, PhysicalRootError> {
    // Pinned Vec2 reserves max(required,2*old,4) exactly. Its two columns
    // share one allocation. Pinned hashbrown 0.14 capacity rounding is <2*(n+1);
    // Index's own bound uses <=2*(full_capacity+1) buckets. Four*(n+1)
    // therefore covers controls+usize buckets. Sixteen words cover Index's
    // HashTable/allocator/high-water header; 31 bytes cover alignment/replicas.
    let entries = Layout::array::<(BodyKey, NativeBodyLease)>(capacity)
        .map_err(|_| PhysicalRootError::Resource)?;
    let hashes =
        Layout::array::<StarlarkHashValue>(capacity).map_err(|_| PhysicalRootError::Resource)?;
    let entries = entries
        .extend(hashes)
        .map_err(|_| PhysicalRootError::Resource)?
        .0
        .size();
    let buckets = add(capacity, 1)?
        .checked_mul(4)
        .ok_or(PhysicalRootError::Resource)?;
    let index = buckets
        .checked_mul(add(std::mem::size_of::<usize>(), 1)?)
        .ok_or(PhysicalRootError::Resource)?;
    add(
        add(entries, index)?,
        add(16 * std::mem::size_of::<usize>(), 31)?,
    )
}
fn body_bytes(length: usize) -> Result<usize, PhysicalRootError> {
    let payload = Layout::array::<u8>(length).map_err(|_| PhysicalRootError::Resource)?;
    Ok(Layout::new::<[usize; 2]>()
        .extend(payload)
        .map_err(|_| PhysicalRootError::Resource)?
        .0
        .pad_to_align()
        .size())
}

impl NativeBodyLedger {
    pub(in crate::runtime) fn new(
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        reservation: &Arc<RepositoryWorkspaceReservation>,
        active: &dyn Fn() -> bool,
    ) -> Result<Arc<Self>, PhysicalRootError> {
        check(scope, active)?;
        if scope.workspace() != reservation.workspace() {
            return Err(PhysicalRootError::Reservation);
        }
        let credit = resources.reserve_body_bytes(arc_bytes::<Self>()?)?;
        let origin_credit = resources.reserve_body_bytes(add(
            arc_bytes::<Origin>()?,
            reservation
                .retained_storage_bytes()
                .map_err(PhysicalRootError::Protocol)?,
        )?)?;
        Ok(Arc::new(Self {
            storage: Mutex::new(BodyMap::new(resources)?),
            origin: Arc::new(Origin {
                resources: resources.clone(),
                scope: scope.clone(),
                reservation: reservation.clone(),
                _credit: origin_credit,
            }),
            _credit: credit,
        }))
    }
    pub(in crate::runtime) fn check_origin(
        &self,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        reservation: &Arc<RepositoryWorkspaceReservation>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        self.origin.check(active)?;
        if !Arc::ptr_eq(resources, &self.origin.resources)
            || !Arc::ptr_eq(scope, &self.origin.scope)
            || !Arc::ptr_eq(reservation, &self.origin.reservation)
        {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
    pub(in crate::runtime) fn candidates(
        self: &Arc<Self>,
    ) -> Result<NativeBodyCandidates, PhysicalRootError> {
        self.origin.check(&|| true)?;
        Ok(NativeBodyCandidates {
            storage: BodyMap::new(&self.origin.resources)?,
            ledger: self.clone(),
            _credit: self
                .origin
                .resources
                .reserve_body_bytes(std::mem::size_of::<NativeBodyCandidates>())?,
        })
    }
    /// Cost association for a borrowed request. Structural reservation equality
    /// permits ordinary foreign Arc aliases; full current/request authentication
    /// remains with the command before any IO or process admission.
    pub(in crate::runtime) fn check_request_origin(
        &self,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        reservation: &Arc<RepositoryWorkspaceReservation>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        self.origin.check(active)?;
        if !Arc::ptr_eq(resources, &self.origin.resources)
            || !Arc::ptr_eq(scope, &self.origin.scope)
            || reservation.as_ref() != self.origin.reservation.as_ref()
        {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
    pub(in crate::runtime) fn lookup(
        &self,
        body: &Arc<[u8]>,
        active: &dyn Fn() -> bool,
    ) -> Result<Option<NativeBodyLease>, PhysicalRootError> {
        self.origin.check(active)?;
        Ok(self
            .storage
            .lock()
            .map_err(|_| PhysicalRootError::Resource)?
            .bodies
            .get(&key(body))
            .cloned())
    }
    /// Borrow candidates so refusal cannot retire coverage before caller-held
    /// raw request/result/transcript owners. The visitor must enumerate ONLY
    /// bodies of the successfully completed, accepted canonical transcript.
    pub(in crate::runtime) fn commit_canonical(
        &self,
        candidates: &mut NativeBodyCandidates,
        mut visit: impl FnMut(&mut Visitor<'_>) -> Result<(), PhysicalRootError>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        self.origin.check(active)?;
        if !std::ptr::eq(self, candidates.ledger.as_ref()) {
            return Err(PhysicalRootError::Reservation);
        }
        let mut selected = BodyMap::new(&self.origin.resources)?;
        visit(&mut |body| {
            self.origin.check(active)?;
            if self.lookup(body, active)?.is_none() {
                let lease = candidates
                    .storage
                    .bodies
                    .get(&key(body))
                    .ok_or(PhysicalRootError::Reservation)?;
                selected.insert(lease.clone())?;
            }
            Ok(())
        })?;
        self.origin.check(active)?;
        let mut storage = self
            .storage
            .lock()
            .map_err(|_| PhysicalRootError::Resource)?;
        storage.ensure(selected.bodies.len())?;
        for (k, lease) in selected.bodies {
            // Preserve an already published lease if another synchronous
            // publisher won this key after the borrowed validation walk.
            if !storage.bodies.contains_key(&k) {
                storage.bodies.insert(k, lease);
            }
        }
        Ok(())
    }
    pub(in crate::runtime) fn final_set(
        &self,
        mut visit: impl FnMut(&mut Visitor<'_>) -> Result<(), PhysicalRootError>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeBodySet, PhysicalRootError> {
        let mut storage = BodyMap::new(&self.origin.resources)?;
        visit(&mut |body| {
            self.origin.check(active)?;
            storage.insert(
                self.lookup(body, active)?
                    .ok_or(PhysicalRootError::Reservation)?,
            )
        })?;
        self.origin.check(active)?;
        Ok(NativeBodySet {
            storage,
            origin: self.origin.clone(),
            _credit: self
                .origin
                .resources
                .reserve_body_bytes(std::mem::size_of::<NativeBodySet>())?,
        })
    }
}
impl NativeBodyCandidates {
    pub(in crate::runtime) fn ledger(&self) -> &Arc<NativeBodyLedger> {
        &self.ledger
    }

    /// Share already admitted temporary owners. `other` stays intact until its
    /// corresponding raw semantic/capture owners have retired.
    pub(in crate::runtime) fn absorb(
        &mut self,
        other: &Self,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        self.ledger.origin.check(active)?;
        if !Arc::ptr_eq(&self.ledger, &other.ledger) {
            return Err(PhysicalRootError::Reservation);
        }
        self.storage.ensure(other.storage.bodies.len())?;
        for (key, lease) in &other.storage.bodies {
            self.ledger.origin.check(active)?;
            if !self.storage.bodies.contains_key(key) {
                self.storage.bodies.insert(*key, lease.clone());
            }
        }
        Ok(())
    }
    pub(in crate::runtime) fn lookup(
        &self,
        body: &Arc<[u8]>,
        active: &dyn Fn() -> bool,
    ) -> Result<Option<NativeBodyLease>, PhysicalRootError> {
        self.ledger.origin.check(active)?;
        if let Some(lease) = self.storage.bodies.get(&key(body)) {
            return Ok(Some(lease.clone()));
        }
        self.ledger.lookup(body, active)
    }
    pub(in crate::runtime) fn retain(
        &mut self,
        body: &Arc<[u8]>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeBodyLease, PhysicalRootError> {
        if let Some(lease) = self.lookup(body, active)? {
            return Ok(lease);
        }
        let pending = self.reserve_new(body.len(), active)?;
        self.finish(pending, body.clone(), active)
    }
    pub(in crate::runtime) fn reserve_new(
        &mut self,
        length: usize,
        active: &dyn Fn() -> bool,
    ) -> Result<PendingNativeBody, PhysicalRootError> {
        match self.reserve_new_for_capture(length, active)? {
            NativeBodyAdmission::New(pending) => Ok(pending),
            NativeBodyAdmission::Refused(_) => Err(PhysicalRootError::Resource),
        }
    }
    pub(in crate::runtime) fn finish(
        &mut self,
        pending: PendingNativeBody,
        body: Arc<[u8]>,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeBodyLease, PhysicalRootError> {
        // Explicit owner order covers all early returns and freeze failures.
        struct Incoming {
            body: Arc<[u8]>,
            pending: PendingNativeBody,
        }
        let incoming = Incoming { body, pending };
        self.ledger.origin.check(active)?;
        if !Arc::ptr_eq(&incoming.pending.origin, &self.ledger.origin)
            || incoming.body.len() != incoming.pending.length
        {
            return Err(PhysicalRootError::Reservation);
        }
        let Incoming { body, pending } = incoming;
        let lease = NativeBodyLease(
            pending
                .credit
                .freeze(body)
                .map_err(|_| PhysicalRootError::Resource)?,
        );
        self.storage.insert(lease.clone())?;
        Ok(lease)
    }
    /// A supplied witness must cover this exact allocation and native bank.
    /// Unknown ordinary bodies instead use retain's fully charged admission.
    pub(in crate::runtime) fn retain_claimed(
        &self,
        body: &Arc<[u8]>,
        witness: &NativeBodyLease,
        active: &dyn Fn() -> bool,
    ) -> Result<NativeBodyLease, PhysicalRootError> {
        self.ledger.origin.check(active)?;
        if !Arc::ptr_eq(body, witness.body())
            || !self.ledger.origin.resources.owns_body_block(&witness.0)
        {
            return Err(PhysicalRootError::Reservation);
        }
        // A lease alone cannot transfer a foreign root's coverage association.
        let known = self
            .lookup(body, active)?
            .ok_or(PhysicalRootError::Reservation)?;
        if !std::ptr::eq(known.0.get(), witness.0.get()) {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(known)
    }
}
impl NativeBodySet {
    pub(in crate::runtime) fn lookup(
        &self,
        body: &Arc<[u8]>,
        active: &dyn Fn() -> bool,
    ) -> Result<Option<NativeBodyLease>, PhysicalRootError> {
        self.origin.check(active)?;
        Ok(self.storage.bodies.get(&key(body)).cloned())
    }
    pub(in crate::runtime) fn check_origin(
        &self,
        resources: &Arc<RepositoryRootResources>,
        scope: &Arc<SourceServiceScope>,
        reservation: &Arc<RepositoryWorkspaceReservation>,
        active: &dyn Fn() -> bool,
    ) -> Result<(), PhysicalRootError> {
        self.origin.check(active)?;
        if !Arc::ptr_eq(resources, &self.origin.resources)
            || !Arc::ptr_eq(scope, &self.origin.scope)
            || !Arc::ptr_eq(reservation, &self.origin.reservation)
        {
            return Err(PhysicalRootError::Reservation);
        }
        Ok(())
    }
}
macro_rules! counts_debug {
    ($t:ty, $name:literal) => {
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct($name).finish_non_exhaustive()
            }
        }
    };
}
counts_debug!(NativeBodyLedger, "NativeBodyLedger");
counts_debug!(NativeBodyCandidates, "NativeBodyCandidates");
counts_debug!(NativeBodySet, "NativeBodySet");
counts_debug!(PendingNativeBody, "PendingNativeBody");

#[cfg(test)]
mod tests;
