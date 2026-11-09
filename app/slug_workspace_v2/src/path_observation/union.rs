//! Left-biased union of already validated immutable observation epochs.
use starlark_map::ordered_map::OrderedMap;

use super::*;

impl PathObservationEpoch {
    /// Retain every fact from both epochs, refusing the first sorted conflicting
    /// demand. Equal duplicates retain the complete left key/result pair, which
    /// can own different image credits from an equal right-hand observation.
    /// Empty, shared-owner and right-subset unions reuse an existing epoch.
    pub fn try_union(&self, other: &Self) -> Result<Self, PathObservationEpochError> {
        if other.observations.is_empty() || Arc::ptr_eq(&self.observations, &other.observations) {
            return Ok(self.dupe());
        }
        if self.observations.is_empty() {
            return Ok(other.dupe());
        }

        // Inputs have already passed the constructors' operation/image checks.
        // Their only possible disagreement is a conflicting shared demand.
        // Validate all overlaps before allocating, in sorted conflict order.
        let mut additions = 0usize;
        for (demand, result) in other.observations.iter() {
            match self.get(demand) {
                Some(original)
                    if !Arc::ptr_eq(original, result) && original.as_ref() != result.as_ref() =>
                {
                    return Err(PathObservationEpochError::ConflictingDemand(demand.dupe()));
                }
                Some(_) => {}
                None => additions += 1,
            }
        }
        if additions == 0 {
            return Ok(self.dupe());
        }

        // Both input maps are live allocations with positive-size rows, so their
        // combined row count fits the address space. Keep arithmetic explicit;
        // allocation failure retains the existing constructors' behavior.
        let capacity = self
            .observations
            .len()
            .checked_add(additions)
            .expect("two live observation maps have addressable combined rows");
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
        Ok(Self {
            observations: Arc::new(SortedMap::from(merged)),
        })
    }
}

#[cfg(test)]
#[path = "union_tests.rs"]
mod tests;
