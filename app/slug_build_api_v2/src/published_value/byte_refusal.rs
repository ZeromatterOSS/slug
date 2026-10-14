//! Bounded admission-time forecasts; no retained native objects or caller data.
use super::*;

/// The immediate byte charge, independent of any enclosing recursive walk.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PublicationByteContext {
    Local,
    ImportBody,
    OwnerBody,
    SharedBody,
}

/// Immutable context for one refused `charge_bytes` request.
///
/// These are admitted construction forecasts, including paid producer scratch;
/// they are not allocator measurements or a physical-memory decomposition.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PublicationByteRefusal {
    context: PublicationByteContext,
    used_bytes: usize,
    requested_bytes: usize,
    limit_bytes: usize,
    checked_total_bytes: Option<usize>,
    prior_poisoned: bool,
    producer_scratch_bytes: usize,
    import_bytes: usize,
    owner_bytes: usize,
    shared_bytes: usize,
    producer_work: Option<usize>,
}

impl PublicationByteRefusal {
    pub fn context(&self) -> PublicationByteContext {
        self.context
    }
    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }
    pub fn requested_bytes(&self) -> usize {
        self.requested_bytes
    }
    pub fn limit_bytes(&self) -> usize {
        self.limit_bytes
    }
    pub fn checked_total_bytes(&self) -> Option<usize> {
        self.checked_total_bytes
    }
    pub fn prior_poisoned(&self) -> bool {
        self.prior_poisoned
    }
    pub fn producer_scratch_bytes(&self) -> usize {
        self.producer_scratch_bytes
    }
    pub fn import_bytes(&self) -> usize {
        self.import_bytes
    }
    pub fn owner_bytes(&self) -> usize {
        self.owner_bytes
    }
    pub fn shared_bytes(&self) -> usize {
        self.shared_bytes
    }
    pub fn producer_work(&self) -> Option<usize> {
        self.producer_work
    }
}

impl PublishedValueArenaBuilder {
    // Called only after the primitive's original refusal has poisoned this builder.
    pub(super) fn byte_refusal(
        &self,
        requested: usize,
        prior_poisoned: bool,
    ) -> PublishedValueError {
        PublishedValueError::ByteBudget(Box::new(PublicationByteRefusal {
            context: PublicationByteContext::Local,
            used_bytes: self.bytes,
            requested_bytes: requested,
            limit_bytes: BYTE_LIMIT,
            checked_total_bytes: self.bytes.checked_add(requested),
            prior_poisoned,
            producer_scratch_bytes: self.producer_scratch_bytes,
            import_bytes: self.import_bytes,
            owner_bytes: self.owner_payload.bytes,
            shared_bytes: self.shared_payload.bytes,
            producer_work: self.visits.checked_sub(self.owner_payload.visits),
        }))
    }
}

impl PublishedValueError {
    pub(super) fn is_limit(&self) -> bool {
        matches!(self, Self::Limit(_) | Self::ByteBudget(_))
    }

    // Decorate only the directly refused body; never overwrite an inner context.
    pub(super) fn with_byte_context(mut self, context: PublicationByteContext) -> Self {
        if let Self::ByteBudget(refusal) = &mut self {
            if refusal.context == PublicationByteContext::Local {
                refusal.context = context;
            }
        }
        self
    }
}

#[cfg(test)]
#[path = "byte_refusal/tests.rs"]
mod tests;
