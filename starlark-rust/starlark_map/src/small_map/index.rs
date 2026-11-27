//! Allocation certificates for the private optional lookup index.

use std::alloc;
use std::alloc::Layout;
use std::mem;
use std::ptr::NonNull;

use allocative::Allocative;
use hashbrown::HashTable;
use hashbrown::TryReserveError;

use super::BoundedMapError;

// Pinned hashbrown 0.14.5 raw generic/SSE2/NEON group implementations.
const MIN_GROUP_WIDTH: usize = 4;
const MAX_GROUP_WIDTH: usize = 16;

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
        // Match hashbrown 0.14.5 RawTableInner::capacity_to_buckets for a
        // requested capacity. The retained-storage certificate below starts
        // from a different fact, the effective capacity observed after allocation.
        let buckets = match capacity {
            0 => return Some((0, mem::size_of::<Self>())),
            1..=3 => 4,
            4..=7 => 8,
            n => (n.checked_mul(8)? / 7).checked_next_power_of_two()?,
        };
        let control_len = buckets.checked_add(MAX_GROUP_WIDTH)?;
        let controls = Layout::from_size_align(control_len, MAX_GROUP_WIDTH).ok()?;
        let (layout, _) = Layout::array::<usize>(buckets)
            .ok()?
            .extend(controls)
            .ok()?;
        Some((
            control_len,
            layout.size().checked_add(mem::size_of::<Self>())?,
        ))
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
        // Pinned hashbrown 0.14.5's triangular probe visits disjoint control
        // groups until finding a vacant bucket. Each failed group contains
        // at least MIN_GROUP_WIDTH live entries. Include the successful
        // group and the possible extra small-table fixup load.
        charge(occupied / MIN_GROUP_WIDTH + 2, 0).map_err(BoundedMapError::Operation)?;
        self.table
            .insert_unique(hash, value, |_| unreachable!("fresh index must not grow"));
        Ok(())
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
