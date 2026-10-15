//! Bounded failure-only reads of completed guard certificates, never payloads.
use super::*;

#[path = "census/native.rs"]
mod native;
pub use native::PublicationNativeChannel;
pub use native::PublicationNativeRetentionCensus;
pub use native::PublicationNativeRootFamily;
pub use native::PublicationNativeStatistics;
pub use native::PublicationRowKind;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct PublicationOwnerDomainCensus {
    count: usize,
    intrinsic_visits: usize,
    maximum_intrinsic_visits: usize,
}
impl PublicationOwnerDomainCensus {
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn intrinsic_visits(&self) -> usize {
        self.intrinsic_visits
    }
    pub fn maximum_intrinsic_visits(&self) -> usize {
        self.maximum_intrinsic_visits
    }
    fn add(&mut self, visits: usize) -> Option<()> {
        self.count = self.count.checked_add(1)?;
        self.intrinsic_visits = self.intrinsic_visits.checked_add(visits)?;
        self.maximum_intrinsic_visits = self.maximum_intrinsic_visits.max(visits);
        Some(())
    }
}
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct PublicationClosureCensus {
    bzl_load: PublicationOwnerDomainCensus,
    configured_rule: PublicationOwnerDomainCensus,
    configured_aspect: PublicationOwnerDomainCensus,
    imported_arenas: usize,
    imported_local_visits: usize,
    source_references: usize,
    imported_source_body_facts: usize,
    unique_sources: usize,
    unique_source_body_facts: usize,
    native_retention: Option<PublicationNativeRetentionCensus>,
}
impl PublicationClosureCensus {
    pub fn native_retention(&self) -> Option<&PublicationNativeRetentionCensus> {
        self.native_retention.as_ref()
    }
    pub fn owners(&self, domain: FunctionOwnerDomain) -> &PublicationOwnerDomainCensus {
        match domain {
            FunctionOwnerDomain::BzlLoad => &self.bzl_load,
            FunctionOwnerDomain::ConfiguredRule => &self.configured_rule,
            FunctionOwnerDomain::ConfiguredAspect => &self.configured_aspect,
        }
    }
    pub fn imported_arenas(&self) -> usize {
        self.imported_arenas
    }
    pub fn imported_local_visits(&self) -> usize {
        self.imported_local_visits
    }
    pub fn source_references(&self) -> usize {
        self.source_references
    }
    pub fn imported_source_body_facts(&self) -> usize {
        self.imported_source_body_facts
    }
    pub fn unique_sources(&self) -> usize {
        self.unique_sources
    }
    pub fn unique_source_body_facts(&self) -> usize {
        self.unique_source_body_facts
    }
}

// One positive observer record is consumed BEFORE each cached read. This is
// fixed error-handling work, not a change to either semantic admission ledger.
struct Observer {
    remaining: usize,
}
impl Observer {
    fn before_read(&mut self) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        Some(())
    }
}
fn envelope(b: &PublishedValueArenaBuilder, total: usize) -> Option<usize> {
    // Iterators scan empty hash buckets too: guard their capacities first.
    if b.owner_seen
        .capacity()
        .checked_add(b.closure_seen.capacity())?
        > NODE_LIMIT.checked_mul(4)?
    {
        return None;
    }
    let work = b.checked_producer_work()?;
    if !b.strict_shared_audit
        || b.failed_budget
        || b.certified_shared_visits != 0
        || work > NODE_LIMIT
        || b.shared_payload.visits > NODE_LIMIT
        || !b.owner_active.is_empty()
        || !b.arena_active.is_empty()
        || !b.rows.is_empty()
        || !b.edges.is_empty()
        || !b.slots.is_empty()
        || b.source_seen.len() != b.sources.len()
        || b.visits.checked_add(b.import_visits)? != total
        || b.import_visits.checked_add(b.owner_payload.visits)? > RETAINED_FACT_LIMIT
    {
        return None;
    }
    let records = work.checked_mul(2)?;
    let known = b
        .owner_seen
        .len()
        .checked_add(b.closure_seen.len())?
        .checked_add(b.sources.len())?;
    (known <= records).then_some(records)
}
pub(super) fn collect(
    b: &PublishedValueArenaBuilder,
    total: usize,
) -> Option<PublicationClosureCensus> {
    let records = envelope(b, total)?;
    observe(b, &mut Observer { remaining: records })
}
fn observe(
    b: &PublishedValueArenaBuilder,
    observer: &mut Observer,
) -> Option<PublicationClosureCensus> {
    let mut out = PublicationClosureCensus::default();
    let mut rows = Some(0usize);
    for owner in b.owner_seen.values() {
        observer.before_read()?;
        let visits = owner.intrinsic_shape().visits;
        match owner.domain() {
            FunctionOwnerDomain::BzlLoad => out.bzl_load.add(visits)?,
            FunctionOwnerDomain::ConfiguredRule => out.configured_rule.add(visits)?,
            FunctionOwnerDomain::ConfiguredAspect => out.configured_aspect.add(visits)?,
        }
    }
    for identity in &b.closure_seen {
        observer.before_read()?;
        let arena = &identity.0;
        // Same prepaid fixed arena header read; inability affects native only.
        rows = rows.and_then(|n| n.checked_add(arena.0.rows.len()));
        out.imported_arenas = out.imported_arenas.checked_add(1)?;
        out.imported_local_visits = out
            .imported_local_visits
            .checked_add(arena.0.local_shape.visits)?;
        for source in arena.sources() {
            observer.before_read()?;
            out.source_references = out.source_references.checked_add(1)?;
            out.imported_source_body_facts = out
                .imported_source_body_facts
                .checked_add(source.fact_count())?;
        }
    }
    for source in &b.sources {
        observer.before_read()?;
        out.unique_sources = out.unique_sources.checked_add(1)?;
        out.unique_source_body_facts = out
            .unique_source_body_facts
            .checked_add(source.fact_count())?;
    }
    let owners = out
        .bzl_load
        .intrinsic_visits
        .checked_add(out.configured_rule.intrinsic_visits)?
        .checked_add(out.configured_aspect.intrinsic_visits)?;
    if owners != b.owner_payload.visits
        || out.imported_local_visits != b.import_visits
        || out.unique_sources > out.source_references
        || out.unique_source_body_facts > out.imported_source_body_facts
        || out.imported_source_body_facts > out.imported_local_visits
    {
        return None;
    }
    out.native_retention = native::collect(b, rows, out.imported_source_body_facts);
    Some(out)
}

#[cfg(test)]
#[path = "census/tests.rs"]
pub(super) mod tests;
