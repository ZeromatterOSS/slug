//! Allocation certificates for the private optional lookup index.

use std::mem;

use allocative::Allocative;
use hashbrown::HashTable;

#[derive(Clone, Allocative)]
pub(super) struct Index {
    table: HashTable<usize>,
    // HashTable::capacity is only items+growth_left: tombstones can lower it
    // without releasing buckets. Fresh/grown allocations expose full capacity.
    full_capacity_high_water: usize,
}

// Refresh even if a future internal hasher unwinds after allocating a table.
struct Refresh<'a>(&'a mut Index);
impl Drop for Refresh<'_> {
    fn drop(&mut self) {
        self.0.full_capacity_high_water =
            self.0.full_capacity_high_water.max(self.0.table.capacity());
    }
}

impl Index {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        let table = HashTable::with_capacity(capacity);
        Self {
            full_capacity_high_water: table.capacity(),
            table,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.table.len()
    }

    pub(super) fn find(&self, hash: u64, eq: impl FnMut(&usize) -> bool) -> Option<&usize> {
        self.table.find(hash, eq)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &usize> {
        self.table.iter()
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut usize> {
        self.table.iter_mut()
    }

    pub(super) fn reserve(&mut self, additional: usize, hasher: impl Fn(&usize) -> u64) {
        let refresh = Refresh(self);
        refresh.0.table.reserve(additional, hasher);
    }

    pub(super) fn insert_unique(
        &mut self,
        hash: u64,
        value: usize,
        hasher: impl Fn(&usize) -> u64,
    ) {
        let refresh = Refresh(self);
        refresh.0.table.insert_unique(hash, value, hasher);
    }

    pub(super) fn remove(&mut self, hash: u64, eq: impl FnMut(&usize) -> bool) -> Option<usize> {
        // Return only the removed value, never a vacant entry that could grow
        // the table without refreshing the allocation certificate.
        self.table
            .find_entry(hash, eq)
            .ok()
            .map(|entry| entry.remove().0)
    }

    pub(super) fn retain(&mut self, predicate: impl FnMut(&mut usize) -> bool) {
        self.table.retain(predicate);
    }

    pub(super) fn clear(&mut self) {
        self.table.clear();
    }

    pub(super) fn retained_storage_bytes_upper_bound(&self) -> Option<usize> {
        // Pinned hashbrown 0.14: full capacity c implies buckets <= 2*(c+1).
        // Its usize table has one control byte/bucket, <=16 replicated control
        // bytes, and at most max(alignof(usize),16)-1 alignment padding.
        let buckets = self
            .full_capacity_high_water
            .checked_add(1)?
            .checked_mul(2)?;
        let payload = buckets.checked_mul(mem::size_of::<usize>().checked_add(1)?)?;
        mem::size_of::<Self>()
            .checked_add(payload)?
            .checked_add(mem::align_of::<usize>().max(16) - 1)?
            .checked_add(16)
    }
}

#[cfg(test)]
#[path = "index_storage_tests.rs"]
mod tests;
