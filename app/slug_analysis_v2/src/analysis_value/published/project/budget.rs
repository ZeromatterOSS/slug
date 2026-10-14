//! One conservative phase ledger for descriptor owners and projection work.
use super::*;

#[derive(Default)]
pub(super) struct ProjectionBudget {
    used: usize,
    arena_map_credit: usize,
    arenas: UnorderedMap<PublishedArenaIdentity, ()>,
    failed: bool,
}
impl ProjectionBudget {
    pub(super) fn poison(&mut self) {
        self.failed = true;
    }
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        match self
            .used
            .checked_add(bytes)
            .filter(|n| *n <= BYTE_LIMIT && !self.failed)
        {
            Some(next) => {
                self.used = next;
                Ok(())
            }
            None => {
                self.failed = true;
                Err("combined native projection byte limit".into())
            }
        }
    }
    fn admission<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, String>,
    ) -> Result<T, String> {
        if self.failed {
            return Err("poisoned native projection budget".into());
        }
        let result = f(self);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub(super) fn admit_arena(&mut self, arena: &PublishedValueArena) -> Result<(), String> {
        self.admission(|budget| {
            let identity = arena.identity(); // cheap guard; no payload clone/allocation
            if budget.arenas.contains_key(&identity) {
                return Ok(());
            }
            let count = budget
                .arenas
                .len()
                .checked_add(1)
                .filter(|n| *n <= NODE_LIMIT)
                .ok_or("projection arena count limit")?;
            let next = map_peak::<PublishedArenaIdentity, ()>(count)?;
            let delta = next
                .checked_sub(budget.arena_map_credit)
                .ok_or("projection map forecast")?;
            budget.charge(
                arena
                    .shape()
                    .bytes
                    .checked_add(delta)
                    .ok_or("projection arena forecast")?,
            )?;
            budget.arena_map_credit = next;
            budget.arenas.insert(identity, ());
            Ok(())
        })
    }
    /// Forecast and admission share poisoning, including checked-arithmetic errors.
    pub(super) fn reserve_forecast(
        &mut self,
        credit: &mut usize,
        forecast: impl FnOnce() -> Result<usize, String>,
    ) -> Result<(), String> {
        self.admission(|budget| budget.reserve_batch(credit, forecast()?))
    }
    /// No refunds: finished batches retain their conservative native/caller peak.
    pub(super) fn reserve_batch(&mut self, credit: &mut usize, bytes: usize) -> Result<(), String> {
        if bytes > *credit {
            self.charge(bytes - *credit)?;
            *credit = bytes;
        } else if self.failed {
            return Err("poisoned native projection budget".into());
        }
        Ok(())
    }
}

/// Pinned hashbrown0.14.5: load/rounding use <4*N buckets (minimum4);
/// old+new rehash allocations plus controls/alignment fit the conservative8*N
/// cells below. Inline table header and small/empty minima are included.
fn map_peak<K, V>(count: usize) -> Result<usize, String> {
    let cell = std::mem::size_of::<(K, V)>()
        .checked_add(16)
        .and_then(|n| n.checked_add(std::mem::align_of::<(K, V)>()))
        .ok_or("projection map forecast")?;
    count
        .max(4)
        .checked_mul(8)
        .and_then(|n| n.checked_mul(cell))
        .and_then(|n| n.checked_add(std::mem::size_of::<UnorderedMap<K, V>>()))
        .ok_or_else(|| "projection map forecast".into())
}
fn vec_peak<T>(count: usize) -> Result<usize, String> {
    count
        .max(4)
        .checked_mul(4)
        .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
        .and_then(|n| n.checked_add(std::mem::size_of::<Vec<T>>()))
        .ok_or_else(|| "projection vector forecast".into())
}
/// Covers explicit DFS frames, local/active maps, originals and input rows,
/// segmented successor snapshots, persistent native memo and outward-value memo.
pub(super) fn caller_peak(nodes: usize, edges: usize) -> Result<usize, String> {
    let mut total = 0usize;
    let frames = nodes.checked_add(edges).ok_or("projection frame count")?;
    for cost in [
        vec_peak::<(PublishedValueRef, bool)>(frames)?,
        map_peak::<PublishedValueIdentity, usize>(nodes)?,
        map_peak::<PublishedValueIdentity, ()>(nodes)?,
        map_peak::<PublishedValueIdentity, AnalysisDepset>(nodes)?,
        map_peak::<PublishedValueIdentity, AnalysisValue>(nodes)?,
        vec_peak::<PublishedValueRef>(nodes)?,
        vec_peak::<AnalysisDepsetGraphNode>(nodes)?,
        vec_peak::<AnalysisDepsetGraphInput>(edges)?,
        nodes
            .checked_mul(std::mem::size_of::<Vec<AnalysisDepsetGraphInput>>())
            .ok_or("projection segmented inputs")?,
    ] {
        total = total
            .checked_add(cost)
            .ok_or("projection caller forecast")?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    #[derive(Debug)]
    struct Originals;
    impl PublishedOriginalBinding for Originals {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn root_count(&self) -> usize {
            1
        }
        fn imported_arenas(&self) -> &[PublishedValueArena] {
            &[]
        }
    }
    fn arena() -> PublishedValueArena {
        let mut b = PublishedValueArenaBuilder::new();
        let node = b.reserve().unwrap();
        let atom = b.native_atom(AnalysisValue::none()).unwrap();
        b.set_node(node, PublishedNode::Native(atom)).unwrap();
        b.set_original_slot(node, PublishedRootSlot::new(0).unwrap())
            .unwrap();
        b.seal(Arc::new(Originals)).unwrap()
    }
    #[test]
    fn phase_budget_counts_arena_once_and_distinct_equal_arena_again() {
        let first = arena();
        let second = arena();
        let mut b = ProjectionBudget::default();
        b.admit_arena(&first).unwrap();
        let used = b.used;
        b.admit_arena(&first.clone()).unwrap();
        assert_eq!(b.used, used);
        b.admit_arena(&second).unwrap();
        assert_eq!(b.used, used + second.shape().bytes);
        assert_eq!(b.arenas.len(), 2);
    }
    #[test]
    fn phase_budget_carries_nested_and_completed_peaks_without_extra_pool() {
        let first = arena();
        let mut b = ProjectionBudget::default();
        b.admit_arena(&first).unwrap();
        b.charge(BYTE_LIMIT - b.used - 1024).unwrap();
        let mut outer = 0;
        let mut nested = 0;
        b.reserve_batch(&mut outer, 512).unwrap();
        b.reserve_batch(&mut nested, 256).unwrap();
        // Growing the outer batch sees the nested batch's retained credit.
        assert!(b.reserve_batch(&mut outer, 769).is_err());
        assert!(b.failed);
        assert!(b.admit_arena(&first).is_err());
        assert!(b.reserve_batch(&mut nested, 1).is_err());
    }
    #[test]
    fn failed_forecast_poison_refuses_cached_value_and_prior_arena() {
        let arena = arena();
        let original = arena.value(PublishedNodeId::new(0).unwrap()).unwrap();
        let mut projector = Projector::default();
        projector.value(&original).unwrap();
        let mut credit = 0;
        assert!(
            projector
                .budget
                .reserve_forecast(&mut credit, || caller_peak(usize::MAX, 1))
                .is_err()
        );
        assert!(projector.budget.failed);
        assert!(projector.budget.admit_arena(&arena).is_err());
        assert!(projector.value(&original).is_err());
    }
    #[test]
    fn caller_forecast_covers_empty_small_allocations_and_checked_overflow() {
        assert!(caller_peak(0, 0).unwrap() > 0);
        assert!(caller_peak(1, 0).unwrap() >= caller_peak(0, 0).unwrap());
        assert!(caller_peak(0, 1).unwrap() >= caller_peak(0, 0).unwrap());
        assert!(caller_peak(usize::MAX, 1).is_err());
        assert!(
            map_peak::<PublishedValueIdentity, AnalysisDepset>(1).unwrap()
                > 4 * std::mem::size_of::<(PublishedValueIdentity, AnalysisDepset)>()
        );
    }
    #[test]
    fn joint_projection_budget_accepts_former_limit_and_checks_exact_overflow_and_poison() {
        assert_eq!(BYTE_LIMIT, 128 * 1024 * 1024);
        let arena = arena();
        let mut budget = ProjectionBudget::default();
        budget.admit_arena(&arena).unwrap();
        let original = budget.used;
        assert!(
            original > arena.shape().bytes,
            "arena-map storage shares the ledger"
        );
        budget.charge(64 * 1024 * 1024 - original).unwrap();
        assert_eq!(budget.used, 64 * 1024 * 1024);
        let mut outer = 0;
        let mut nested = 0;
        budget.reserve_batch(&mut outer, 512).unwrap();
        budget.reserve_batch(&mut nested, 256).unwrap();
        budget.charge(BYTE_LIMIT - budget.used).unwrap();
        assert_eq!(budget.used, BYTE_LIMIT);
        budget.admit_arena(&arena).unwrap();
        assert!(budget.reserve_batch(&mut nested, 257).is_err());
        assert_eq!(nested, 256);
        assert_eq!(budget.used, BYTE_LIMIT);
        assert!(budget.admit_arena(&arena).is_err());
        assert!(budget.charge(0).is_err());

        let mut overflow = ProjectionBudget::default();
        overflow.admit_arena(&arena).unwrap();
        let before = overflow.used;
        assert!(overflow.charge(usize::MAX).is_err());
        assert_eq!(overflow.used, before);
        assert!(overflow.admit_arena(&arena).is_err());
    }
}
