//! Storage and probe bounds for fresh hashbrown 0.14.5 tables.

use std::alloc::Layout;

// Pinned raw generic/SSE2/NEON implementations.
const MIN_GROUP_WIDTH: usize = 4;
const MAX_GROUP_WIDTH: usize = 16;

/// Control initialization work and allocation bytes for a newly reserved table.
/// The byte bound includes the widest supported control group and alignment.
pub fn fresh_allocation<T>(capacity: usize) -> Option<(usize, usize)> {
    let buckets = match capacity {
        0 => return Some((0, 0)),
        1..=3 => 4,
        4..=7 => 8,
        n => (n.checked_mul(8)? / 7).checked_next_power_of_two()?,
    };
    let control_len = buckets.checked_add(MAX_GROUP_WIDTH)?;
    let controls = Layout::from_size_align(control_len, MAX_GROUP_WIDTH).ok()?;
    let (layout, _) = Layout::array::<T>(buckets).ok()?.extend(controls).ok()?;
    Some((control_len, layout.size()))
}

/// Probe work for an insertion into a fully reserved table without deletions.
pub fn fresh_insert_work(occupied: usize) -> usize {
    // Each earlier probe group is full. Include the terminal group and the
    // possible extra control load when inserting into a small table.
    occupied / MIN_GROUP_WIDTH + 2
}
