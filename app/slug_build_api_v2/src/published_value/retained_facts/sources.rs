use super::*;

pub(super) struct LocalSources {
    seen: fxhash::FxHashSet<usize>,
    values: Vec<CallableSourceAuthority>,
}
impl LocalSources {
    pub(super) fn new(rows: usize, meter: &mut Meter<'_>) -> Result<Self, PublishedValueError> {
        meter.work()?;
        // Hash table buckets/control bytes, old+new allocation peak, plus both
        // Vec/Arc handle arrays at transfer. No growth is admitted after this.
        let buckets = rows
            .saturating_mul(2)
            .checked_next_power_of_two()
            .unwrap_or(usize::MAX)
            .max(4);
        let table_scratch = buckets
            .saturating_mul(std::mem::size_of::<usize>() + 1)
            .saturating_add(32)
            .saturating_mul(2);
        // Only the temporary table allocation is classified. Retain its fixed
        // header and every handle/transfer credit for the final source array.
        let retained = std::mem::size_of::<fxhash::FxHashSet<usize>>().saturating_add(
            rows.saturating_mul(3 * std::mem::size_of::<CallableSourceAuthority>()),
        );
        meter.byte_parts(retained, table_scratch)?;
        Ok(Self {
            seen: fxhash::FxHashSet::with_capacity_and_hasher(rows, Default::default()),
            values: Vec::with_capacity(rows),
        })
    }
    pub(super) fn observe(
        &mut self,
        source: &CallableSourceAuthority,
        meter: &mut Meter<'_>,
    ) -> Result<(), PublishedValueError> {
        meter.work()?;
        if !self.seen.insert(source.storage_identity()) {
            return Ok(());
        }
        // The complete draft rows hold owning authority guards throughout this
        // scratch lookup; actual addresses are never semantic source identity.
        // The immutable constructor already admitted this complete count.
        // Reading its private certificate avoids recounting source payloads;
        // the FULL count still contributes to canonical retained visits.
        meter.work()?;
        meter.bytes(std::mem::size_of::<usize>())?;
        meter.add(source.fact_count())?;
        self.values.push(source.clone());
        Ok(())
    }
    pub(super) fn values(&self) -> &[CallableSourceAuthority] {
        &self.values
    }
    pub(super) fn finish(self) -> Vec<CallableSourceAuthority> {
        self.values
    }
}
