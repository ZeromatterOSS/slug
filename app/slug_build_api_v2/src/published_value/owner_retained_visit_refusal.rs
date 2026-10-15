//! Complete scalar context at the strict owner constructor's final visit gate.
use super::*;

#[path = "owner_retained_visit_refusal/census.rs"]
mod census;
pub use census::PublicationClosureCensus;
pub use census::PublicationNativeChannel;
pub use census::PublicationNativeRetentionCensus;
pub use census::PublicationNativeRootFamily;
pub use census::PublicationNativeStatistics;
pub use census::PublicationOwnerDomainCensus;
pub use census::PublicationRowKind;

/// Admitted accounting units and encounter history, not unique physical storage.
/// Strict C is zero. Complete retained visits are D + O + ALLS + I + R;
/// the independent legacy audit history remains V + I = W + O + I.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PublicationOwnerRetainedVisitRefusal {
    domain: FunctionOwnerDomain,
    stable_key_visits: usize,
    input_validity_visits: usize,
    intrinsic_visits: usize,
    legacy_local_visits: usize,
    owner_visits: usize,
    imported_visits: usize,
    shared_visits: usize,
    certified_shared_visits: usize,
    producer_work: Option<usize>,
    declaration_reference_visits: usize,
    complete_visits: usize,
    limit_visits: usize,
    closure_census: Option<PublicationClosureCensus>,
}
impl PublicationOwnerRetainedVisitRefusal {
    pub fn closure_census(&self) -> Option<&PublicationClosureCensus> {
        self.closure_census.as_ref()
    }
    pub fn domain(&self) -> FunctionOwnerDomain {
        self.domain
    }
    pub fn stable_key_visits(&self) -> usize {
        self.stable_key_visits
    }
    pub fn input_validity_visits(&self) -> usize {
        self.input_validity_visits
    }
    pub fn intrinsic_visits(&self) -> usize {
        self.intrinsic_visits
    }
    pub fn legacy_local_visits(&self) -> usize {
        self.legacy_local_visits
    }
    pub fn owner_visits(&self) -> usize {
        self.owner_visits
    }
    pub fn imported_visits(&self) -> usize {
        self.imported_visits
    }
    pub fn shared_visits(&self) -> usize {
        self.shared_visits
    }
    pub fn certified_shared_visits(&self) -> usize {
        self.certified_shared_visits
    }
    pub fn producer_work(&self) -> Option<usize> {
        self.producer_work
    }
    pub fn declaration_reference_visits(&self) -> usize {
        self.declaration_reference_visits
    }
    pub fn complete_visits(&self) -> usize {
        self.complete_visits
    }
    pub fn limit_visits(&self) -> usize {
        self.limit_visits
    }
}
impl PublishedValueArenaBuilder {
    // Only the final reached Owner::new predicate calls this in production.
    // It borrows completed counters and cannot retain a part or dependency.
    pub(in crate::published_value) fn owner_retained_visit_refusal(
        &self,
        domain: FunctionOwnerDomain,
        stable_key_visits: usize,
        input_validity_visits: usize,
        intrinsic_visits: usize,
        declaration_reference_visits: usize,
        complete_visits: usize,
    ) -> PublishedValueError {
        PublishedValueError::OwnerRetainedVisitBudget(Box::new(
            PublicationOwnerRetainedVisitRefusal {
                domain,
                stable_key_visits,
                input_validity_visits,
                intrinsic_visits,
                legacy_local_visits: self.visits,
                owner_visits: self.owner_payload.visits,
                imported_visits: self.import_visits,
                shared_visits: self.shared_payload.visits,
                certified_shared_visits: self.certified_shared_visits,
                producer_work: self.checked_producer_work(),
                declaration_reference_visits,
                complete_visits,
                limit_visits: RETAINED_FACT_LIMIT,
                closure_census: self
                    .visits
                    .checked_add(self.import_visits)
                    .and_then(|legacy| census::collect(self, legacy)),
            },
        ))
    }
}

#[cfg(test)]
#[path = "owner_retained_visit_refusal/tests.rs"]
mod tests;
