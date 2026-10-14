//! One checked record encounter with an optional reservation on its paid owner.
use super::PublishedNodeId;
use super::PublishedValueArenaBuilder;
use super::PublishedValueError;

/// A borrow-scoped, single-use admission for one publication record.
///
/// The builder remains exclusively borrowed. Only byte admission is exposed;
/// callers cannot replace the owner or detach its paid admission. Dropping a
/// record keeps its paid visit and reserves no row, as required for memo hits.
#[must_use]
pub struct PublicationRecord<'a> {
    builder: &'a mut PublishedValueArenaBuilder,
}

impl PublishedValueArenaBuilder {
    /// Observe depth and pay for one record before inspecting its memo entry.
    /// An optional row reservation through the returned record shares this unit.
    pub fn begin_record(
        &mut self,
        depth: usize,
    ) -> Result<PublicationRecord<'_>, PublishedValueError> {
        self.observe_depth(depth)?;
        self.charge_visit()?;
        Ok(PublicationRecord { builder: self })
    }
}

impl PublicationRecord<'_> {
    /// Admit caller storage before allocating it or reserving the optional row.
    pub fn charge_bytes(&mut self, bytes: usize) -> Result<(), PublishedValueError> {
        self.builder.charge_bytes(bytes)
    }

    /// Borrow byte-only classification for storage dropped before publication.
    pub fn producer_scratch(&mut self) -> super::PublicationScratch<'_> {
        self.builder.producer_scratch()
    }

    /// Consume this paid encounter to reserve one row on the same builder.
    /// A failed reservation poisons the unpublished builder, without a refund.
    pub fn reserve(self) -> Result<PublishedNodeId, PublishedValueError> {
        let result = self.builder.reserve_impl(false);
        if result.is_err() {
            self.builder.failed_budget = true;
        }
        result
    }

    /// Abandon an unsuccessful encounter and poison its builder without refund.
    /// Successful memo hits instead drop the record without calling this method.
    pub fn abort(self) {
        self.builder.failed_budget = true;
    }
}

#[cfg(test)]
#[path = "record/tests.rs"]
mod tests;
