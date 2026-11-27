use std::cell::Cell;
use std::hash::Hash;
use std::hash::Hasher;

use super::BoundedMapError;
use super::Hashed;
use super::NO_INDEX_THRESHOLD;
use super::SmallMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Colliding(usize);

impl Hash for Colliding {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        0u8.hash(hasher);
    }
}

fn rows(count: usize) -> impl Iterator<Item = (Hashed<Colliding>, usize)> {
    (0..count).rev().map(|i| (Hashed::new(Colliding(i)), 2 * i))
}

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Work,
    Allocation,
}

#[test]
fn fallible_unique_rows_admit_storage_before_callbacks_and_stop_on_error() {
    let count = NO_INDEX_THRESHOLD + 1;
    let visited = Cell::new(0);
    let make_rows = || {
        rows(count).enumerate().map(|(index, row)| {
            visited.set(visited.get() + 1);
            if index == 1 {
                Err(Refusal::Work)
            } else {
                Ok(row)
            }
        })
    };
    let result = SmallMap::try_from_unique_hashed_fallible_rows(count, make_rows(), |_, bytes| {
        if bytes > 0 {
            Err(Refusal::Allocation)
        } else {
            Ok(())
        }
    });
    assert_eq!(
        result.unwrap_err(),
        BoundedMapError::Operation(Refusal::Allocation)
    );
    assert_eq!(visited.get(), 0);

    let result = SmallMap::try_from_unique_hashed_fallible_rows(count, make_rows(), |_, _| Ok(()));
    assert_eq!(
        result.unwrap_err(),
        BoundedMapError::Operation(Refusal::Work)
    );
    assert_eq!(visited.get(), 2);
}

#[test]
fn bounded_rows_admit_exact_work_and_bytes_across_index_threshold() {
    for count in [0, NO_INDEX_THRESHOLD, NO_INDEX_THRESHOLD + 1, 65] {
        let mut work = 0;
        let mut bytes = 0;
        let map = SmallMap::try_from_unique_hashed_rows(count, rows(count), |used, allocated| {
            work += used;
            bytes += allocated;
            Ok::<_, Refusal>(())
        })
        .unwrap();
        let expected = rows(count).map(|(key, value)| (key.into_key(), value));
        assert_eq!(
            map.iter()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            expected.collect::<Vec<_>>(),
        );
        assert_eq!(map.capacity(), count);
        assert_eq!(map.index.is_some(), count > NO_INDEX_THRESHOLD);
        map.assert_invariants();

        let limited = |mut work: usize, mut bytes: usize, observed: &Cell<usize>| {
            SmallMap::try_from_unique_hashed_rows(
                count,
                rows(count).inspect(|_| observed.set(observed.get() + 1)),
                |used, allocated| {
                    work = work.checked_sub(used).ok_or(Refusal::Work)?;
                    bytes = bytes.checked_sub(allocated).ok_or(Refusal::Allocation)?;
                    Ok(())
                },
            )
        };
        assert!(
            limited(work, bytes, &Cell::new(0))
                .unwrap()
                .eq_ordered(&map)
        );
        assert_eq!(
            limited(work - 1, bytes, &Cell::new(0)).unwrap_err(),
            BoundedMapError::Operation(Refusal::Work),
        );
        if bytes != 0 {
            let observed = Cell::new(0);
            assert_eq!(
                limited(work, bytes - 1, &observed).unwrap_err(),
                BoundedMapError::Operation(Refusal::Allocation),
            );
            assert_eq!(observed.get(), 0);
        }
    }
}

#[test]
fn collision_charges_cover_index_probes_and_map_remains_ordinary() {
    let count = 65;
    let mut work = 0;
    let mut observed = SmallMap::try_from_unique_hashed_rows(count, rows(count), |used, _| {
        work += used;
        Ok::<_, Refusal>(())
    })
    .unwrap();
    let mut ordinary = SmallMap::with_capacity(count);
    for (key, value) in rows(count) {
        ordinary.insert_hashed_unique_unchecked(key, value);
    }
    // A requested 65 rows needs 128 buckets. This includes every admitted
    // insertion's probe bound, even with all keys in a single collision run.
    assert_eq!(
        work,
        128 + 16 + 1 + (0..count).map(|i| 3 + i / 4).sum::<usize>()
    );
    assert!(observed.eq_ordered(&ordinary));
    assert_eq!(observed.get(&Colliding(0)), Some(&0));
    assert_eq!(observed.get(&Colliding(count)), None);

    for map in [&mut observed, &mut ordinary] {
        assert_eq!(map.shift_remove(&Colliding(27)), Some(54));
        map.reverse();
        assert_eq!(map.pop(), Some((Colliding(count - 1), 2 * (count - 1))));
        for i in count..2 * count {
            assert_eq!(map.insert(Colliding(i), 2 * i), None);
        }
        map.retain(|key, _| key.0 % 3 == 0);
        map.sort_keys();
        map.assert_invariants();
        assert!(map.clone().eq_ordered(map));
    }
    assert!(observed.eq_ordered(&ordinary));
}

#[test]
fn bounded_rows_refuse_mismatched_counts_and_unrepresentable_storage() {
    for (declared, actual) in [(0, 1), (2, 1), (1, 2)] {
        assert_eq!(
            SmallMap::try_from_unique_hashed_rows(declared, rows(actual), |_, _| {
                Ok::<_, Refusal>(())
            })
            .unwrap_err(),
            BoundedMapError::RowCountMismatch,
        );
    }
    assert_eq!(
        SmallMap::try_from_unique_hashed_rows(usize::MAX, rows(0), |_, _| -> Result<(), Refusal> {
            panic!("an invalid layout must not reach the budget")
        })
        .unwrap_err(),
        BoundedMapError::SizeOverflow,
    );
}

#[test]
fn bounded_overwrites_match_ordinary_order_under_full_hash_collisions() {
    for count in [4, NO_INDEX_THRESHOLD + 2] {
        let input = || rows(count - 1).chain([(Hashed::new(Colliding(count - 2)), count)]);
        let mut expected = SmallMap::with_capacity(count);
        for (key, value) in input() {
            expected.insert_hashed(key, value);
        }
        let mut bytes = 0;
        let map = SmallMap::try_from_hashed_rows(
            count,
            input().map(Ok),
            |left, right| Ok(left == right),
            |_, allocated| {
                bytes += allocated;
                Ok::<_, Refusal>(())
            },
        )
        .unwrap();
        assert!(map.eq_ordered(&expected));
        assert_eq!(map.len(), count - 1);
        assert_eq!(map.get(&Colliding(count - 2)), Some(&count));
        assert_eq!(map.index.is_some(), count > NO_INDEX_THRESHOLD);
        map.assert_invariants();

        let produced = Cell::new(0);
        let error = SmallMap::try_from_hashed_rows(
            count,
            input()
                .inspect(|_| produced.set(produced.get() + 1))
                .map(Ok),
            |left, right| Ok(left == right),
            |_, allocated| {
                if allocated >= bytes {
                    Err(Refusal::Allocation)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert_eq!(error, BoundedMapError::Operation(Refusal::Allocation));
        assert_eq!(produced.get(), 0);
    }
}

#[test]
fn bounded_overwrites_stop_before_more_rows_after_an_indexed_comparison_refuses() {
    let count = NO_INDEX_THRESHOLD + 2;
    let produced = Cell::new(0);
    let compared = Cell::new(0);
    let error = SmallMap::try_from_hashed_rows(
        count,
        rows(count)
            .inspect(|_| produced.set(produced.get() + 1))
            .map(Ok),
        |_, _| {
            compared.set(compared.get() + 1);
            Err(Refusal::Work)
        },
        |_, _| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error, BoundedMapError::Operation(Refusal::Work));
    assert_eq!((produced.get(), compared.get()), (2, 1));

    let input = rows(count).map(Ok).chain([Err(Refusal::Work)]);
    let error = SmallMap::try_from_hashed_rows(
        count,
        input,
        |left, right| Ok(left == right),
        |_, _| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error, BoundedMapError::Operation(Refusal::Work));
}
