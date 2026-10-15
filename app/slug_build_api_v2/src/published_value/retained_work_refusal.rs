//! Fixed scalar context for only seal's local canonical work refusal.
use super::*;

/// V/O/C are entry ledger values while the local W loan advances. These are
/// accounting counters, not physical objects or measured memory. The closed phase only attributes failure.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PublicationRetainedCertificateWorkRefusal {
    phase: retained_facts::CertificationPhase,
    entry_work: usize,
    work_at_refusal: usize,
    legacy_visits: usize,
    owner_facts: usize,
    certified_shared_facts: usize,
    shared_facts: usize,
    rows_len: usize,
    edges_len: usize,
    slots_len: usize,
    owner_roots_len: usize,
    shared_seen_len: usize,
    imports_len: usize,
}
impl PublicationRetainedCertificateWorkRefusal {
    pub fn context(&self) -> &'static str {
        "sealLocalCertification"
    }
    pub fn phase(&self) -> &'static str {
        self.phase.label()
    }
    pub fn limit_work(&self) -> usize {
        NODE_LIMIT
    }
    pub fn checked_added_work(&self) -> Option<usize> {
        self.work_at_refusal.checked_sub(self.entry_work)
    }
    pub fn entry_work(&self) -> usize {
        self.entry_work
    }
    pub fn work_at_refusal(&self) -> usize {
        self.work_at_refusal
    }
    pub fn legacy_visits(&self) -> usize {
        self.legacy_visits
    }
    pub fn owner_facts(&self) -> usize {
        self.owner_facts
    }
    pub fn certified_shared_facts(&self) -> usize {
        self.certified_shared_facts
    }
    pub fn shared_facts(&self) -> usize {
        self.shared_facts
    }
    pub fn rows_len(&self) -> usize {
        self.rows_len
    }
    pub fn edges_len(&self) -> usize {
        self.edges_len
    }
    pub fn slots_len(&self) -> usize {
        self.slots_len
    }
    pub fn owner_roots_len(&self) -> usize {
        self.owner_roots_len
    }
    pub fn shared_seen_len(&self) -> usize {
        self.shared_seen_len
    }
    pub fn imports_len(&self) -> usize {
        self.imports_len
    }
}
impl PublishedValueArenaBuilder {
    // Called only after the original local_visits error has poisoned the
    // consumed builder. No callbacks, walks, clones or allocation on success.
    pub(super) fn retained_certificate_work_refusal(
        &self,
        entry_work: usize,
        work_at_refusal: usize,
        phase: retained_facts::CertificationPhase,
    ) -> PublishedValueError {
        PublishedValueError::RetainedCertificateWork(Box::new(
            PublicationRetainedCertificateWorkRefusal {
                phase,
                entry_work,
                work_at_refusal,
                legacy_visits: self.visits,
                owner_facts: self.owner_payload.visits,
                certified_shared_facts: self.certified_shared_visits,
                shared_facts: self.shared_payload.visits,
                rows_len: self.rows.len(),
                edges_len: self.edges.len(),
                slots_len: self.slots.len(),
                owner_roots_len: self.owner_roots.len(),
                shared_seen_len: self.shared_seen.len(),
                imports_len: self.imports.len(),
            },
        ))
    }
}

#[cfg(test)]
#[path = "retained_work_refusal/tests.rs"]
mod tests;
