use std::alloc::Layout;
use std::mem;

use hashbrown::raw::RawTable;

use super::Index;
use crate::StarlarkHashValue;
use crate::small_map::BoundedMapError;
use crate::small_map::SmallMap;
use crate::small_set::SmallSet;

#[test]
fn fresh_index_forecast_covers_the_pinned_requested_capacity_layout() {
    for count in [1, 3, 7, 8, 17, 29, 56, 57, 65, 128] {
        let actual = RawTable::<usize>::try_with_capacity(count).unwrap();
        let (_, layout) = actual.allocation_info();
        let (_, forecast) = Index::fresh_allocation(count).unwrap();
        assert!(
            forecast >= layout.size() + mem::size_of::<Index>(),
            "{count}"
        );
    }
}

#[test]
fn collision_probe_refusal_precedes_inserting_the_fresh_slot() {
    let count = 29;
    let mut index = Index::try_with_capacity::<()>(count).unwrap();
    for i in 0..count - 1 {
        index
            .insert_fresh(0, i, &mut |_, _| Ok::<_, ()>(()))
            .unwrap();
    }

    let charge = (count - 1) / 4 + 2;
    assert_eq!(
        index.insert_fresh(0, count - 1, &mut |work, bytes| {
            assert_eq!((work, bytes), (charge, 0));
            Err(())
        }),
        Err(BoundedMapError::Operation(())),
    );
    assert_eq!(index.len(), count - 1);
    assert_eq!(index.find(0, |value| *value == count - 1), None);
    index
        .insert_fresh(0, count - 1, &mut |work, bytes| {
            assert_eq!((work, bytes), (charge, 0));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        index.find(0, |value| *value == count - 1),
        Some(&(count - 1))
    );
}

#[test]
fn sparse_reserved_insert_uses_the_occupied_control_group_bound() {
    let mut index = Index::with_capacity(65);
    let mut remaining = 1usize;
    let mut charge = |work, bytes| {
        assert_eq!(bytes, 0);
        remaining = remaining.checked_sub(work).ok_or("work")?;
        Ok(())
    };
    assert_eq!(
        index.insert_reserved_bounded(7, 12, &mut charge),
        Err(BoundedMapError::Operation("work"))
    );
    assert_eq!(index.len(), 0);

    let mut remaining = 2usize;
    index
        .insert_reserved_bounded(7, 12, &mut |work, bytes| -> Result<(), &'static str> {
            assert_eq!(bytes, 0);
            remaining = remaining.checked_sub(work).ok_or("work")?;
            Ok(())
        })
        .unwrap();
    assert_eq!(remaining, 0);
    assert_eq!(index.find(7, |value| *value == 12), Some(&12));
}

#[test]
fn empty_reserved_clone_keeps_hidden_index_storage_certificate() {
    let empty = SmallSet::<usize>::new();
    assert_eq!(empty.retained_storage_bytes_upper_bound(), Some(0));

    let reserved = SmallSet::<usize>::with_capacity(4096);
    let cloned = reserved.clone();
    assert!(cloned.is_empty());
    assert_eq!(cloned.capacity(), 0);
    assert!(cloned.retained_storage_bytes_upper_bound().unwrap() > 4096 * mem::size_of::<usize>());
    assert!(
        reserved.retained_storage_bytes_upper_bound() > cloned.retained_storage_bytes_upper_bound()
    );

    let map = SmallMap::<usize, ()>::with_capacity(4096);
    let cloned = map.clone();
    let original_index = map.index.as_ref().unwrap();
    let cloned_index = cloned.index.as_ref().unwrap();
    assert_eq!(
        cloned_index.full_capacity_high_water,
        original_index.full_capacity_high_water
    );
    assert_eq!(
        cloned.retained_storage_bytes_upper_bound(),
        cloned_index.retained_storage_bytes_upper_bound()
    );
}

#[test]
fn tombstones_growth_clear_and_rebuild_preserve_index_certificate() {
    let mut index = Index::with_capacity(64);
    let initial_capacity = index.full_capacity_high_water;
    for value in 0..initial_capacity {
        index.insert_unique(0, value, |_| 0);
    }
    let initial_bound = index.retained_storage_bytes_upper_bound().unwrap();
    index
        .charge_lookup_probe(&mut |work, bytes| {
            assert_eq!((work, bytes), (index.len() / 4 + 2, 0));
            Ok::<_, ()>(())
        })
        .unwrap();
    // A long collision run forces deleted control bytes rather than releasing
    // backing storage, so the current capacity ceases to certify that storage.
    for value in 0..initial_capacity / 2 {
        assert_eq!(
            index.remove(0, |candidate| *candidate == value),
            Some(value)
        );
    }
    assert!(index.table.capacity() < initial_capacity);
    assert_eq!(index.full_capacity_high_water, initial_capacity);
    index
        .charge_lookup_probe(&mut |work, bytes| {
            assert_eq!((work, bytes), (2 * (initial_capacity + 1) / 4 + 2, 0));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(
        index.retained_storage_bytes_upper_bound(),
        Some(initial_bound)
    );
    let mut cloned = index.clone();
    assert_eq!(cloned.full_capacity_high_water, initial_capacity);
    assert_eq!(
        cloned.retained_storage_bytes_upper_bound(),
        Some(initial_bound)
    );
    cloned.reserve(initial_capacity * 4, |_| 0);
    assert!(cloned.full_capacity_high_water > initial_capacity);
    assert!(cloned.retained_storage_bytes_upper_bound().unwrap() > initial_bound);
    let grown_bound = cloned.retained_storage_bytes_upper_bound();
    cloned.clear();
    assert_eq!(cloned.retained_storage_bytes_upper_bound(), grown_bound);

    let mut inserted = Index::with_capacity(1);
    let small_capacity = inserted.full_capacity_high_water;
    for value in 0..=small_capacity {
        inserted.insert_unique(0, value, |_| 0);
    }
    assert!(inserted.full_capacity_high_water > small_capacity);

    let mut map = SmallMap::<usize, usize>::with_capacity(64);
    for value in 0..64 {
        map.insert(value, value * 2);
    }
    let old_high_water = map.index.as_ref().unwrap().full_capacity_high_water;
    map.retain(|key, _| *key % 3 == 0);
    map.assert_invariants();
    assert_eq!(
        map.index.as_ref().unwrap().full_capacity_high_water,
        old_high_water
    );
    for value in (0..64).step_by(3) {
        assert_eq!(map.get(&value), Some(&(value * 2)));
    }
    let before_clear = map.retained_storage_bytes_upper_bound();
    map.clear();
    assert_eq!(map.retained_storage_bytes_upper_bound(), before_clear);
    assert!(map.clone().retained_storage_bytes_upper_bound().unwrap() > 0);
}

#[test]
fn storage_certificate_uses_vec2_layout_and_checked_index_arithmetic() {
    let map = SmallMap::<[u8; 3], u64>::with_capacity(3);
    assert!(map.index.is_none());
    let entries = Layout::array::<([u8; 3], u64)>(map.capacity()).unwrap();
    let hashes = Layout::array::<StarlarkHashValue>(map.capacity()).unwrap();
    assert_eq!(
        map.retained_storage_bytes_upper_bound(),
        Some(entries.extend(hashes).unwrap().0.size())
    );
    // The extra scalar lives in the optional boxed index, not every map.
    assert_eq!(
        mem::size_of::<SmallMap<usize, usize>>(),
        mem::size_of::<crate::vec_map::VecMap<usize, usize>>()
            + mem::size_of::<Option<Box<Index>>>()
    );
    let mut index = Index::with_capacity(0);
    index.full_capacity_high_water = usize::MAX;
    assert_eq!(index.retained_storage_bytes_upper_bound(), None);
}
