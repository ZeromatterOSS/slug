//! Left-biased union of already validated immutable observation epochs.
use starlark_map::ordered_map::OrderedMap;

use super::*;

impl PathObservationEpoch {
    /// Retain every fact from both epochs, refusing the first sorted conflicting
    /// demand. Equal duplicates retain the complete left key/result pair, which
    /// can own different image credits from an equal right-hand observation.
    /// Empty, shared-owner and right-subset unions reuse an existing epoch.
    /// A proper right superset can also be reused when every left key/result
    /// retains exactly the same owners in that already complete right map.
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
            return Ok(self.dupe());
        }
        // The validated maps have unique keys: right rows minus newly added
        // rows equals the number of shared keys. No extra pass/index is needed.
        // The entire conflict pass above still runs after custody is disproved.
        if compatible_superset
            && other.observations.len().checked_sub(additions) == Some(self.observations.len())
        {
            return Ok(other.dupe());
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
