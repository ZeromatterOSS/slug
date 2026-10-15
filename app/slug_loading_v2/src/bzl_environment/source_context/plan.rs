//! Constructor-local borrowed canonical union; no pointer state escapes.
use starlark_map::small_map::SmallMap;

use super::*;
#[path = "plan/accounting.rs"]
mod accounting;
mod conflict;
pub(super) use accounting::identity;
pub(super) use accounting::identity_prefix;
pub(super) use accounting::identity_with_mapping;
pub(super) use accounting::mapping_entries;

pub(super) struct Plan<'a> {
    pub(super) rows: Vec<&'a BzlModuleSourceProvenance>,
    index: SmallMap<u64, Vec<usize>>,
    // The borrowed row guards EVERY pointer key, including noncanonical foreign
    // equal rows, until this plan is dropped. Inputs cannot move or ABA.
    successful: SmallMap<usize, (usize, &'a BzlModuleSourceProvenance)>,
    pub(super) work: Budget,
    remaining: usize,
}
impl<'a> Plan<'a> {
    pub(super) fn new(count: usize, mut work: Budget) -> Result<Self, String> {
        work.charge(1, accounting::scratch_bytes(count)?)?;
        Ok(Self {
            rows: Vec::with_capacity(count),
            index: SmallMap::with_capacity(count),
            successful: SmallMap::with_capacity(count),
            work,
            remaining: count,
        })
    }
    pub(super) fn observe(&mut self, source: &'a BzlModuleSourceProvenance) -> Result<(), String> {
        self.observe_indexed(source, |h| h)
    }
    fn observe_indexed(
        &mut self,
        source: &'a BzlModuleSourceProvenance,
        bucket: impl Fn(u64) -> u64,
    ) -> Result<(), String> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or("source plan encounter bound")?;
        self.work.charge(1, 0)?; // encounter
        self.work.charge(1, 0)?; // successful actual row lookup
        let pointer = source as *const BzlModuleSourceProvenance as usize;
        if self.successful.contains_key(&pointer) {
            return Ok(());
        }
        let key = bucket(accounting::hash_label(
            &source.identity.label,
            &mut self.work,
        )?);
        self.work.charge(1, 0)?; // candidate index lookup
        let mut ordinal = None;
        if let Some(candidates) = self.index.get(&key) {
            for &position in candidates {
                self.work.charge(1, 0)?; // candidate reference
                let prior = self.rows[position];
                if accounting::same_label(
                    &prior.identity.label,
                    &source.identity.label,
                    &mut self.work,
                )? {
                    if let Some(difference) =
                        accounting::difference_tail(prior, source, &mut self.work, || {})?
                    {
                        return Err(conflict::message(
                            &source.identity.label,
                            difference,
                            &mut self.work,
                        )?);
                    }
                    ordinal = Some(position);
                    break;
                }
            }
        }
        let ordinal = if let Some(ordinal) = ordinal {
            ordinal
        } else {
            self.work.charge(2, 0)?; // candidate insert and borrowed row push
            let ordinal = self.rows.len();
            self.index.entry(key).or_default().push(ordinal);
            self.rows.push(source);
            ordinal
        };
        self.work.charge(1, 0)?; // memo insertion, ONLY after complete success
        self.successful.insert(pointer, (ordinal, source));
        Ok(())
    }
}

#[cfg(test)]
#[path = "plan/tests.rs"]
mod tests;
