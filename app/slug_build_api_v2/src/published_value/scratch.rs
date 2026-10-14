//! Trusted classification of already-paid producer-only storage, never a refund.
use super::*;

/// Byte-only admission for storage that drops before a publication is sealed.
///
/// Trusted native producers may classify only reviewed temporary allocations.
/// Original roots and native/source/owner/import payloads stay retained. The
/// capability pays the complete construction forecast on its borrowed builder;
/// it cannot refund, replace or expose that owner. It allocates no storage.
#[must_use]
pub struct PublicationScratch<'a> {
    builder: &'a mut PublishedValueArenaBuilder,
}

impl PublishedValueArenaBuilder {
    /// Borrow admission for producer allocations whose storage ends before seal.
    pub fn producer_scratch(&mut self) -> PublicationScratch<'_> {
        PublicationScratch { builder: self }
    }
}

impl PublicationScratch<'_> {
    /// Pay and classify a wholly temporary construction allocation forecast.
    pub fn charge_bytes(&mut self, scratch: usize) -> Result<(), PublishedValueError> {
        self.charge_parts(0, scratch)
    }

    /// Atomically admit a mixed forecast; classify only its temporary portion.
    pub fn charge_parts(
        &mut self,
        retained: usize,
        scratch: usize,
    ) -> Result<(), PublishedValueError> {
        let (total, next_scratch) = retained
            .checked_add(scratch)
            .zip(self.builder.producer_scratch_bytes.checked_add(scratch))
            .ok_or_else(|| {
                self.builder.failed_budget = true;
                PublishedValueError::Limit("producer scratch bytes")
            })?;
        // No separate allowance: ordinary admission still pays EVERY byte.
        self.builder.charge_bytes(total)?;
        self.builder.producer_scratch_bytes = next_scratch;
        Ok(())
    }
}

#[cfg(test)]
#[path = "scratch/tests.rs"]
mod tests;
