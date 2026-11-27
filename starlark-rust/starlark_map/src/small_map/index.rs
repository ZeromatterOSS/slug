//! Allocation certificates for the private optional lookup index.

use std::alloc;
use std::alloc::Layout;
use std::mem;
use std::ptr::NonNull;

use allocative::Allocative;
use hashbrown::HashTable;
use hashbrown::TryReserveError;

use super::BoundedMapError;
use crate::hashbrown_table;

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

    /// Work to initialize a fresh table and requested bytes for it and its box.
    pub(super) fn fresh_allocation(capacity: usize) -> Option<(usize, usize)> {
        let (work, bytes) = hashbrown_table::fresh_allocation::<usize>(capacity)?;
        Some((work, bytes.checked_add(mem::size_of::<Self>())?))
    }

    /// Allocate a fresh table after the caller pays its layout.
    pub(super) fn try_with_capacity<E>(capacity: usize) -> Result<Box<Self>, BoundedMapError<E>> {
        let mut table = HashTable::new();
        table
            .try_reserve(capacity, |_: &usize| {
                unreachable!("fresh table has no entries")
            })
            .map_err(|error| match error {
                TryReserveError::CapacityOverflow => BoundedMapError::SizeOverflow,
                TryReserveError::AllocError { .. } => BoundedMapError::AllocationFailed,
            })?;
        let index = Self {
            full_capacity_high_water: table.capacity(),
            table,
        };
        // SAFETY: Allocate the exact nonzero Box<Self> layout with the global
        // allocator, verify its pointer, and initialize it before taking ownership.
        let ptr = NonNull::new(unsafe { alloc::alloc(Layout::new::<Self>()) })
            .ok_or(BoundedMapError::AllocationFailed)?
            .cast::<Self>();
        unsafe {
            ptr.as_ptr().write(index);
            Ok(Box::from_raw(ptr.as_ptr()))
        }
    }

    /// Insert into a fully reserved table that has no deletions or tombstones.
    pub(super) fn insert_fresh<E>(
        &mut self,
        hash: u64,
        value: usize,
        charge: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), BoundedMapError<E>> {
        let occupied = self.table.len();
        assert!(
            occupied < self.table.capacity(),
            "fresh index must have spare capacity"
        );
        self.charge_fresh_probe(charge)?;
        self.table
            .insert_unique(hash, value, |_| unreachable!("fresh index must not grow"));
        Ok(())
    }

    pub(super) fn charge_fresh_probe<E>(
        &self,
        charge: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), BoundedMapError<E>> {
        charge(hashbrown_table::fresh_insert_work(self.table.len()), 0)
            .map_err(BoundedMapError::Operation)
    }

    pub(super) fn charge_lookup_probe<E>(
        &self,
        charge: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), BoundedMapError<E>> {
        // Capacity is live entries plus growth credit. Tombstones reduce it
        // below the full capacity witnessed at allocation or completed growth.
        if self.table.capacity() == self.full_capacity_high_water {
            return self.charge_fresh_probe(charge);
        }
        let buckets = self
            .max_bucket_count()
            .ok_or(BoundedMapError::SizeOverflow)?;
        charge(hashbrown_table::lookup_work_upper_bound(buckets), 0)
            .map_err(BoundedMapError::Operation)
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

    pub(super) fn has_spare_capacity(&self) -> bool {
        self.table.len() < self.table.capacity()
    }

    pub(super) fn insert_reserved_bounded<E>(
        &mut self,
        hash: u64,
        value: usize,
        charge: &mut impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), BoundedMapError<E>> {
        assert!(self.has_spare_capacity());
        // A previously used table can contain tombstones. Admit a full
        // probe even if few rows remain, plus the small-table fixup load.
        let work = self
            .max_bucket_count()
            .and_then(|buckets| buckets.checked_add(2))
            .ok_or(BoundedMapError::SizeOverflow)?;
        charge(work, 0).map_err(BoundedMapError::Operation)?;
        // Effective capacity exceeds occupancy, so even insertion into an
        // empty bucket has the required growth credit and cannot rehash.
        self.table
            .insert_unique(hash, value, |_| unreachable!("reserved index cannot grow"));
        Ok(())
    }

    fn max_bucket_count(&self) -> Option<usize> {
        // Pinned hashbrown 0.14: full capacity c implies buckets <= 2*(c+1).
        self.full_capacity_high_water.checked_add(1)?.checked_mul(2)
    }

    pub(super) fn retained_storage_bytes_upper_bound(&self) -> Option<usize> {
        // Its usize table has one control byte/bucket, <=16 replicated control
        // bytes, and at most max(alignof(usize),16)-1 alignment padding.
        let buckets = self.max_bucket_count()?;
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
