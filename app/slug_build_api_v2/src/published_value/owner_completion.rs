//! Positive constructor-only body and declaration-cell completion.
use std::collections::hash_map::Values;

use super::*;

pub(in crate::published_value) const SETUP_WORK: usize = 8;
pub(in crate::published_value) const ROOT_LENGTH_WORK: usize = 6;
pub(in crate::published_value) const OWNER_LENGTH_WORK: usize = 8;
pub(in crate::published_value) const SHARED_LENGTH_WORK: usize = 1;
pub(in crate::published_value) const FINISH_WORK: usize = 6;

pub(in crate::published_value) struct OwnerCompletion {
    pub(in crate::published_value) shape: PublicationShape,
    pub(in crate::published_value) declaration_references: usize,
}

/// Two map iterators, the borrowed roots, result, and fixed scalar/control slots.
pub(in crate::published_value) const fn completion_scratch_bytes() -> usize {
    std::mem::size_of::<OwnerCompletion>()
        + std::mem::size_of::<Values<'_, usize, FunctionEvaluationOwner>>()
        + std::mem::size_of::<Values<'_, usize, SharedPublicationInput>>()
        + std::mem::size_of::<[&dyn StructuralFunctionOwnerPart; 2]>()
        + 16 * std::mem::size_of::<usize>()
}

/// Capacity pays empty-bucket scans too; two units per capacity plus each scan's
/// fixed control conservatively covers the hash iterator's bucket traversal.
pub(in crate::published_value) fn completion_read_work(
    owner_capacity: usize,
    shared_capacity: usize,
    owner_count: usize,
    shared_count: usize,
) -> Option<usize> {
    owner_capacity
        .checked_add(shared_capacity)?
        .checked_mul(2)?
        .checked_add(2)?
        .checked_add(owner_count.checked_mul(OWNER_LENGTH_WORK)?)?
        .checked_add(shared_count.checked_mul(SHARED_LENGTH_WORK)?)?
        .checked_add(ROOT_LENGTH_WORK)?
        .checked_add(FINISH_WORK)
}

fn part_references(part: &dyn StructuralFunctionOwnerPart) -> Option<usize> {
    part.shared_inputs()
        .len()
        .checked_add(part.retained_owners().len())?
        .checked_add(part.imported_arenas().len())
}

impl PublishedValueArenaBuilder {
    /// Only Owner::new supplies its exact audited root parts and original D.
    /// Owning completed memos keep every borrowed declaration guard alive.
    pub(in crate::published_value) fn complete_owner_shape(
        &mut self,
        stable: &dyn StructuralFunctionOwnerPart,
        input: &dyn StructuralFunctionOwnerPart,
        original_intrinsic_visits: usize,
    ) -> Result<OwnerCompletion, PublishedValueError> {
        for _ in 0..SETUP_WORK {
            self.charge_visit()?;
        }
        self.producer_scratch()
            .charge_bytes(completion_scratch_bytes())?;
        if !self.strict_shared_audit
            || self.certified_shared_visits != 0
            || !self.owner_active.is_empty()
            || !self.shared_active.is_empty()
            || !self.arena_active.is_empty()
            || !self.rows.is_empty()
            || !self.edges.is_empty()
            || !self.slots.is_empty()
        {
            self.failed_budget = true;
            return Err(PublishedValueError::Invalid("owner completion state"));
        }
        // Header reads follow setup admission; all iteration and length reads
        // follow their complete positive work envelope. No payload is rescanned.
        let work = completion_read_work(
            self.owner_seen.capacity(),
            self.shared_seen.capacity(),
            self.owner_seen.len(),
            self.shared_seen.len(),
        )
        .ok_or_else(|| {
            self.failed_budget = true;
            PublishedValueError::Limit("owner completion work")
        })?;
        for _ in 0..work {
            self.charge_visit()?;
        }
        let references = (|| {
            let mut count = part_references(stable)?.checked_add(part_references(input)?)?;
            for owner in self.owner_seen.values() {
                count = count
                    .checked_add(part_references(owner.stable_key())?)?
                    .checked_add(part_references(owner.input_validity())?)?;
            }
            for shared in self.shared_seen.values() {
                count = count.checked_add(shared.shared_inputs().len())?;
            }
            Some(count)
        })()
        .ok_or_else(|| {
            self.failed_budget = true;
            PublishedValueError::Limit("owner declaration references")
        })?;
        // Keep legacy arithmetic/strict checks and all admitted non-visit fields.
        let mut shape = self.complete_shape()?;
        shape.visits = original_intrinsic_visits
            .checked_add(self.owner_payload.visits)
            .and_then(|n| n.checked_add(self.shared_payload.visits))
            .and_then(|n| n.checked_add(self.import_visits))
            .and_then(|n| n.checked_add(references))
            .ok_or_else(|| {
                self.failed_budget = true;
                PublishedValueError::Limit("owner complete visits")
            })?;
        Ok(OwnerCompletion {
            shape,
            declaration_references: references,
        })
    }
}
