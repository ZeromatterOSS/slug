//! Left-biased union of already validated immutable observation epochs.
use starlark_map::ordered_map::OrderedMap;

use super::*;

impl PathObservationEpoch {
    /// Assign the exact left-biased union after validating every overlap.
    /// Returned errors preserve all original facts and the map owner. This API
    /// promises valid storage and owner retirement on unwind, not panic rollback;
    /// callers must discard unwound attempt scratch rather than publish it.
    pub fn try_union_assign(&mut self, other: &Self) -> Result<(), PathObservationEpochError> {
        match self.union_plan(other)? {
            UnionPlan::Left => {}
            UnionPlan::Right => *self = other.dupe(),
            UnionPlan::Growth { capacity } => {
                if let Some(map) = Arc::get_mut(&mut self.observations) {
                    // Allocate before taking any LEFT owner. Returned errors
                    // have already been ruled out by the complete plan pass.
                    let mut merged = OrderedMap::with_capacity(capacity);
                    let mut left = std::mem::take(map).into_iter().peekable();
                    let mut right = other.observations.iter().peekable();
                    while let (Some((left_demand, _)), Some((right_demand, _))) =
                        (left.peek(), right.peek())
                    {
                        let row = match left_demand.cmp(right_demand) {
                            Ordering::Less => left.next().expect("peeked left row"),
                            Ordering::Greater => {
                                let (demand, result) = right.next().expect("peeked right row");
                                (demand.dupe(), result.dupe())
                            }
                            Ordering::Equal => {
                                right.next();
                                left.next()
                                    .expect("equal demand retains the complete left row")
                            }
                        };
                        merged.insert(row.0, row.1);
                        #[cfg(test)]
                        assignment_inserted_for_test();
                    }
                    for (demand, result) in left {
                        merged.insert(demand, result);
                        #[cfg(test)]
                        assignment_inserted_for_test();
                    }
                    for (demand, result) in right {
                        merged.insert(demand.dupe(), result.dupe());
                        #[cfg(test)]
                        assignment_inserted_for_test();
                    }
                    debug_assert_eq!(merged.len(), capacity);
                    *map = SortedMap::from(merged);
                } else {
                    // Shared/Weak maps retain the original clone-growth path,
                    // using this plan without a second validation/allocation.
                    *self = self.union_growth(other, capacity);
                }
            }
        }
        Ok(())
    }

    /// Retain every fact from both epochs, refusing the first sorted conflicting
    /// demand. Equal duplicates retain the complete left key/result pair, which
    /// can own different image credits from an equal right-hand observation.
    /// Empty, shared-owner and right-subset unions reuse an existing epoch.
    /// A proper right superset can also be reused when every left key/result
    /// retains exactly the same owners in that already complete right map.
    pub fn try_union(&self, other: &Self) -> Result<Self, PathObservationEpochError> {
        match self.union_plan(other)? {
            UnionPlan::Left => Ok(self.dupe()),
            UnionPlan::Right => Ok(other.dupe()),
            UnionPlan::Growth { capacity } => Ok(self.union_growth(other, capacity)),
        }
    }

    fn union_plan(&self, other: &Self) -> Result<UnionPlan, PathObservationEpochError> {
        if other.observations.is_empty() || Arc::ptr_eq(&self.observations, &other.observations) {
            return Ok(UnionPlan::Left);
        }
        if self.observations.is_empty() {
            return Ok(UnionPlan::Right);
        }

        // Inputs have already passed the constructors' operation/image checks.
        // Their only possible disagreement is a conflicting shared demand.
        // Validate all overlaps before allocating, in sorted conflict order.
        let mut additions = 0usize;
        // Same-size growth cannot contain all left keys; no-new unions retain
        // the left owner first. Avoid custody checks for those impossible cases.
        let mut compatible_superset = other.observations.len() > self.observations.len();
        for (demand, result) in other.observations.iter() {
            match self.observations.get_key_value(demand) {
                Some((_, original))
                    if !Arc::ptr_eq(original, result) && original.as_ref() != result.as_ref() =>
                {
                    return Err(PathObservationEpochError::ConflictingDemand(demand.dupe()));
                }
                Some((original_demand, original)) => {
                    if compatible_superset {
                        compatible_superset = Arc::ptr_eq(original, result)
                            && original_demand.same_retention_owners(demand);
                    }
                }
                None => additions += 1,
            }
        }
        if additions == 0 {
            return Ok(UnionPlan::Left);
        }
        // The validated maps have unique keys: right rows minus newly added
        // rows equals the number of shared keys. No extra pass/index is needed.
        // The entire conflict pass above still runs after custody is disproved.
        if compatible_superset
            && other.observations.len().checked_sub(additions) == Some(self.observations.len())
        {
            return Ok(UnionPlan::Right);
        }

        // Both input maps are live allocations with positive-size rows, so their
        // combined row count fits the address space. Keep arithmetic explicit;
        // allocation failure retains the existing constructors' behavior.
        let capacity = self
            .observations
            .len()
            .checked_add(additions)
            .expect("two live observation maps have addressable combined rows");
        Ok(UnionPlan::Growth { capacity })
    }

    fn union_growth(&self, other: &Self, capacity: usize) -> Self {
        let mut merged = OrderedMap::with_capacity(capacity);
        let mut left = self.observations.iter().peekable();
        let mut right = other.observations.iter().peekable();
        while let (Some((left_demand, _)), Some((right_demand, _))) = (left.peek(), right.peek()) {
            let row = match left_demand.cmp(right_demand) {
                Ordering::Less => left.next().expect("peeked left row"),
                Ordering::Greater => right.next().expect("peeked right row"),
                Ordering::Equal => {
                    right.next();
                    left.next()
                        .expect("equal demand retains the complete left row")
                }
            };
            merged.insert(row.0.dupe(), row.1.dupe());
        }
        for (demand, result) in left.chain(right) {
            merged.insert(demand.dupe(), result.dupe());
        }
        debug_assert_eq!(merged.len(), capacity);
        // SortedMap's existing sorted-key guard avoids sorting/reindexing this
        // already ordered map. There is no intermediate concatenation Vec.
        Self {
            observations: Arc::new(SortedMap::from(merged)),
        }
    }
}

enum UnionPlan {
    Left,
    Right,
    Growth { capacity: usize },
}

// Test-local fault only: the selected insertion completes before one fixed
// unwind. Disarm before panicking so retirement cannot trigger it again.
#[cfg(test)]
std::thread_local! {
    static ASSIGNMENT_UNWIND_AFTER: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
struct AssignmentUnwindForTest;

#[cfg(test)]
impl Drop for AssignmentUnwindForTest {
    fn drop(&mut self) {
        ASSIGNMENT_UNWIND_AFTER.with(|remaining| remaining.set(None));
    }
}

#[cfg(test)]
fn assignment_unwind_after_for_test(insertions: usize) -> AssignmentUnwindForTest {
    assert!(insertions > 0);
    ASSIGNMENT_UNWIND_AFTER.with(|remaining| {
        assert!(remaining.get().is_none());
        remaining.set(Some(insertions));
    });
    AssignmentUnwindForTest
}

#[cfg(test)]
fn assignment_inserted_for_test() {
    let unwind = ASSIGNMENT_UNWIND_AFTER.with(|remaining| match remaining.get() {
        Some(1) => {
            remaining.set(None);
            true
        }
        Some(n) => {
            remaining.set(Some(n - 1));
            false
        }
        None => false,
    });
    if unwind {
        panic!("test-only observation assignment unwind");
    }
}

#[cfg(test)]
#[path = "union_tests.rs"]
mod tests;
