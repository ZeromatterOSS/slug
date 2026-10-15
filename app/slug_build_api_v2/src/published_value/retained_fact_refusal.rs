//! Complete scalar contributions at the final canonical retained-fact gate.
use super::*;

/// Admission-origin facts, not unique physical storage or producer history.
/// C is a subset of shared_facts; the final total is I + O + ALL S + L.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PublicationRetainedFactRefusal {
    imported_facts: usize,
    owner_facts: usize,
    shared_facts: usize,
    local_facts: usize,
    local_source_facts: usize,
    local_native_facts: usize,
    local_other_facts: usize,
    producer_work: usize,
    certified_shared_facts: usize,
    checked_total_facts: Option<usize>,
    limit_facts: usize,
}
impl PublicationRetainedFactRefusal {
    pub fn imported_facts(&self) -> usize {
        self.imported_facts
    }
    pub fn owner_facts(&self) -> usize {
        self.owner_facts
    }
    pub fn shared_facts(&self) -> usize {
        self.shared_facts
    }
    pub fn local_facts(&self) -> usize {
        self.local_facts
    }
    pub fn local_source_facts(&self) -> usize {
        self.local_source_facts
    }
    pub fn local_native_facts(&self) -> usize {
        self.local_native_facts
    }
    pub fn local_other_facts(&self) -> usize {
        self.local_other_facts
    }
    pub fn producer_work(&self) -> usize {
        self.producer_work
    }
    pub fn certified_shared_facts(&self) -> usize {
        self.certified_shared_facts
    }
    pub fn checked_total_facts(&self) -> Option<usize> {
        self.checked_total_facts
    }
    pub fn limit_facts(&self) -> usize {
        self.limit_facts
    }
}
pub(super) fn checked_total(i: usize, o: usize, s: usize, l: usize) -> Option<usize> {
    i.checked_add(o)
        .and_then(|n| n.checked_add(s))
        .and_then(|n| n.checked_add(l))
}
impl PublishedValueArenaBuilder {
    // Only the final gate calls this, after successful local audit and poisoning.
    pub(super) fn retained_fact_refusal(
        &self,
        local: retained_facts::LocalRetainedFacts,
        work: usize,
        checked_total: Option<usize>,
    ) -> PublishedValueError {
        PublishedValueError::RetainedFactBudget(Box::new(PublicationRetainedFactRefusal {
            imported_facts: self.import_visits,
            owner_facts: self.owner_payload.visits,
            shared_facts: self.shared_payload.visits,
            local_facts: local.total,
            local_source_facts: local.source,
            local_native_facts: local.native,
            local_other_facts: local.other(),
            producer_work: work,
            certified_shared_facts: self.certified_shared_visits,
            checked_total_facts: checked_total,
            limit_facts: RETAINED_FACT_LIMIT,
        }))
    }
}
#[cfg(test)]
#[path = "retained_fact_refusal/tests.rs"]
mod tests;
