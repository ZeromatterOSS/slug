//! Failure-only native-envelope observation; every read has a finite record.
use super::*;
use crate::AnalysisValue;
#[path = "native/classifier.rs"]
mod classifier;
#[path = "native/feature.rs"]
mod feature;
#[path = "native/types.rs"]
mod types;
#[path = "native/walk.rs"]
mod walk;
pub use types::PublicationNativeChannel;
pub use types::PublicationNativeRetentionCensus;
pub use types::PublicationNativeRootFamily;
pub use types::PublicationNativeStatistics;
pub use types::PublicationRowKind;

struct Cursor {
    reads: Observer,
    rows: usize,
    providers: usize,
}
impl Cursor {
    fn row(&mut self) -> Option<()> {
        self.reads.before_read()?;
        self.rows = self.rows.checked_sub(1)?;
        Some(())
    }
    fn provider(&mut self) -> Option<()> {
        self.reads.before_read()?;
        self.providers = self.providers.checked_sub(1)?;
        Some(())
    }
    fn certificate(&mut self) -> Option<()> {
        self.reads.before_read()
    }
}
fn allowance(b: &PublishedValueArenaBuilder, rows: Option<usize>) -> Option<(usize, usize)> {
    // The outer observer has already admitted this unchanged capacity envelope.
    // Recheck before the additional independent hash-table passes.
    if b.owner_seen
        .capacity()
        .checked_add(b.closure_seen.capacity())?
        > NODE_LIMIT.checked_mul(4)?
    {
        return None;
    }
    let work = b.checked_producer_work()?;
    let rows = rows?;
    if work > NODE_LIMIT || b.import_visits > RETAINED_FACT_LIMIT || rows > b.import_visits {
        return None;
    }
    // NEW observer2W+4I; with original2W, the total is4W+4I.
    let reads = work
        .checked_mul(2)?
        .checked_add(b.import_visits.checked_mul(4)?)?;
    Some((rows, reads))
}
pub(super) fn collect(
    b: &PublishedValueArenaBuilder,
    rows: Option<usize>,
    sources: usize,
) -> Option<PublicationNativeRetentionCensus> {
    let (rows, reads) = allowance(b, rows)?;
    observe(b, rows, sources, Observer { remaining: reads })
}
fn observe(
    b: &PublishedValueArenaBuilder,
    rows: usize,
    sources: usize,
    reads: Observer,
) -> Option<PublicationNativeRetentionCensus> {
    let mut cursor = Cursor {
        reads,
        rows,
        providers: b.import_visits.checked_sub(rows)?,
    };
    let mut out = PublicationNativeRetentionCensus::default();
    let candidate = walk::first(b, &mut cursor, &mut out)?;
    if cursor.rows != 0 || out.rows != rows {
        return None;
    }
    let channel_sum = out
        .channels
        .iter()
        .try_fold(0usize, |sum, s| sum.checked_add(s.visits))?;
    let family_sum = out
        .families
        .iter()
        .try_fold(0usize, |sum, s| sum.checked_add(s.visits))?;
    if channel_sum != out.native_visits || family_sum != out.native_visits {
        return None;
    }
    out.other_visits = b
        .import_visits
        .checked_sub(sources.checked_add(out.native_visits)?)?;
    candidate.publish(&mut out);
    if out.feature_selected {
        cursor.rows = rows;
        walk::second(b, &mut cursor, &candidate, &mut out)?;
        if cursor.rows != 0 {
            return None;
        }
    }
    Some(out)
}

#[cfg(test)]
#[path = "native/tests.rs"]
mod tests;
