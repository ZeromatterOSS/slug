//! Positive aggregate complement; full standalone caches are never reduced.
use super::*;

pub(super) trait PhysicalAdmission: Sized {
    fn prepare(
        table: &Budget,
        work: &mut Budget,
        base: &Budget,
        owner: PeakOwner,
    ) -> Result<Self, String>;
    fn add_payload(
        &mut self,
        payload: &Budget,
        work: &mut Budget,
        owner: PeakOwner,
    ) -> Result<(), String>;
    fn admit_physical(
        &self,
        owner: PeakOwner,
        phase: PeakPhase,
        base: &Budget,
        full: &Budget,
        work: &mut Budget,
    ) -> Result<(), String>;
    fn admit_state(
        &self,
        owner: PeakOwner,
        phase: PeakPhase,
        base: &Budget,
        logical: &Budget,
        full: &Budget,
        work: &mut Budget,
    ) -> Result<(), String>;
}

/// Zero-sized, statically dispatched legacy path: no new counter or allocation.
pub(super) struct Full;
impl PhysicalAdmission for Full {
    fn prepare(_: &Budget, _: &mut Budget, _: &Budget, _: PeakOwner) -> Result<Self, String> {
        Ok(Self)
    }
    fn add_payload(&mut self, _: &Budget, _: &mut Budget, _: PeakOwner) -> Result<(), String> {
        Ok(())
    }
    fn admit_physical(
        &self,
        owner: PeakOwner,
        phase: PeakPhase,
        base: &Budget,
        full: &Budget,
        work: &mut Budget,
    ) -> Result<(), String> {
        super::admit_combined(owner, phase, FirstLedger::Physical, *base, *full, *work)
    }
    fn admit_state(
        &self,
        owner: PeakOwner,
        phase: PeakPhase,
        base: &Budget,
        logical: &Budget,
        full: &Budget,
        work: &mut Budget,
    ) -> Result<(), String> {
        super::admit_state(owner, phase, *base, *logical, *full, *work)
    }
}

/// Minted only by the private kernel selected by the sealed same-row carrier.
/// Table plus WHOLE first-actual payload bodies; descriptor locals stay in base.
pub(super) struct Covered {
    complement: Budget,
}
impl PhysicalAdmission for Covered {
    fn prepare(
        table: &Budget,
        work: &mut Budget,
        base: &Budget,
        owner: PeakOwner,
    ) -> Result<Self, String> {
        // Fixed state and input/result copies are paid before state construction.
        work.charge_with_context(
            context(owner, PeakPhase::ReferenceTables, FirstLedger::Work),
            2,
            2 * size_of::<Self>(),
        )?;
        let admitted = Self { complement: *table };
        admitted.admit_physical(owner, PeakPhase::ReferenceTables, base, table, work)?;
        Ok(admitted)
    }
    fn add_payload(
        &mut self,
        payload: &Budget,
        work: &mut Budget,
        owner: PeakOwner,
    ) -> Result<(), String> {
        // Copy/read, two scalar additions and the mutation precede no clone.
        work.charge_with_context(
            context(owner, PeakPhase::ReferencePayloadStorage, FirstLedger::Work),
            5,
            2 * size_of::<Budget>(),
        )?;
        self.complement.charge_with_context(
            context(
                owner,
                PeakPhase::ReferencePayloadStorage,
                FirstLedger::Physical,
            ),
            payload.records,
            payload.bytes,
        )
    }
    fn admit_physical(
        &self,
        owner: PeakOwner,
        phase: PeakPhase,
        base: &Budget,
        _: &Budget,
        work: &mut Budget,
    ) -> Result<(), String> {
        // Prepay the new stored scalar read/copy before using the complement.
        work.charge_with_context(
            context(owner, phase, FirstLedger::Work),
            1,
            size_of::<Budget>(),
        )?;
        super::admit_combined(
            owner,
            phase,
            FirstLedger::Physical,
            *base,
            self.complement,
            *work,
        )
    }
    fn admit_state(
        &self,
        owner: PeakOwner,
        phase: PeakPhase,
        base: &Budget,
        logical: &Budget,
        full: &Budget,
        work: &mut Budget,
    ) -> Result<(), String> {
        // Original logical check is first, before any added physical-half work.
        super::admit_combined(
            owner,
            phase,
            FirstLedger::Logical,
            *base,
            *logical,
            Budget::default(),
        )?;
        self.admit_physical(owner, phase, base, full, work)
    }
}
const fn context(owner: PeakOwner, phase: PeakPhase, ledger: FirstLedger) -> PeakContext {
    PeakContext::new(owner, phase, ledger)
}

#[cfg(test)]
#[path = "covered/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "covered/plan_tests.rs"]
mod plan_tests;
