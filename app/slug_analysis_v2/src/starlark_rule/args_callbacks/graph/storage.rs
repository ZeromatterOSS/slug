//! Monotone scratch allocation credits charged to the publication byte owner.
use super::*;

/// Static byte-only admission keeps the same PeakCredit policy usable while
/// the one-use record holds the builder borrow. Neither implementation exposes
/// a mutable owner or an independent resource allowance.
pub(super) trait ByteBudget {
    fn charge_bytes(&mut self, bytes: usize) -> Result<(), String>;
}
impl ByteBudget for PublishedValueArenaBuilder {
    fn charge_bytes(&mut self, bytes: usize) -> Result<(), String> {
        PublishedValueArenaBuilder::charge_bytes(self, bytes).map_err(|e| e.to_string())
    }
}
impl ByteBudget for slug_build_api_v2::published_value::PublicationRecord<'_> {
    fn charge_bytes(&mut self, bytes: usize) -> Result<(), String> {
        slug_build_api_v2::published_value::PublicationRecord::charge_bytes(self, bytes)
            .map_err(|e| e.to_string())
    }
}

impl ByteBudget for slug_build_api_v2::published_value::PublicationScratch<'_> {
    fn charge_bytes(&mut self, bytes: usize) -> Result<(), String> {
        slug_build_api_v2::published_value::PublicationScratch::charge_bytes(self, bytes)
            .map_err(|e| e.to_string())
    }
}

#[derive(Default)]
pub(super) struct PeakCredit(usize);
impl PeakCredit {
    fn admit(
        &mut self,
        graph: &mut impl ByteBudget,
        required: Option<usize>,
    ) -> Result<(), String> {
        let required = match required {
            Some(bytes) => bytes,
            None => {
                graph.charge_bytes(usize::MAX)?;
                return Err("scratch allocation forecast overflow".into());
            }
        };
        graph.charge_bytes(required.saturating_sub(self.0))?;
        self.0 = self.0.max(required);
        Ok(())
    }
    pub(super) fn vector<T>(
        &mut self,
        graph: &mut impl ByteBudget,
        count: usize,
    ) -> Result<(), String> {
        self.admit(
            graph,
            count
                .max(4)
                .checked_mul(4)
                .and_then(|n| n.checked_mul(std::mem::size_of::<T>()))
                .and_then(|n| n.checked_add(std::mem::size_of::<Vec<T>>())),
        )
    }
    /// Pinned hashbrown0.14.5 load/rounding plus old/new rehash coexistence:
    /// 8*N cells (minimum4), each with controls/alignment allowance, and header.
    pub(super) fn map<K, V>(
        &mut self,
        graph: &mut impl ByteBudget,
        count: usize,
    ) -> Result<(), String> {
        let cell = std::mem::size_of::<(K, V)>()
            .checked_add(16)
            .and_then(|n| n.checked_add(std::mem::align_of::<(K, V)>()));
        self.admit(
            graph,
            count
                .max(4)
                .checked_mul(8)
                .and_then(|n| cell.and_then(|cell| n.checked_mul(cell)))
                .and_then(|n| n.checked_add(std::mem::size_of::<UnorderedMap<K, V>>())),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_vector_growth_refuses_before_capacity_or_credit_mutates() {
        let mut graph = PublishedValueArenaBuilder::new();
        graph
            .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - 1)
            .unwrap();
        let mut credit = PeakCredit::default();
        let mut values = Vec::<u64>::new();
        let admitted = credit.vector::<u64>(&mut graph, 1);
        if admitted.is_ok() {
            values.push(1);
        }
        assert!(admitted.is_err());
        assert_eq!(values.capacity(), 0);
        assert_eq!(credit.0, 0);
        assert!(graph.reserve().is_err());
    }
    #[test]
    fn first_map_growth_refuses_before_entry_or_credit_mutates() {
        let mut graph = PublishedValueArenaBuilder::new();
        graph
            .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - 1)
            .unwrap();
        let mut credit = PeakCredit::default();
        let mut values = UnorderedMap::<usize, usize>::new();
        let admitted = credit.map::<usize, usize>(&mut graph.producer_scratch(), 1);
        if admitted.is_ok() {
            values.insert(0, 0);
        }
        assert!(admitted.is_err());
        assert!(values.is_empty());
        assert_eq!(credit.0, 0);
        assert!(graph.reserve().is_err());
    }
    #[test]
    fn record_map_scratch_pays_same_total_and_later_growth_refuses_before_mutation() {
        let mut reference = PublishedValueArenaBuilder::new();
        let mut expected = PeakCredit::default();
        expected.map::<u64, u64>(&mut reference, 4).unwrap();
        let first = reference.charged_bytes();
        expected.map::<u64, u64>(&mut reference, 5).unwrap();
        let growth = reference.charged_bytes() - first;
        assert!(growth > 0);
        let mut graph = PublishedValueArenaBuilder::new();
        let mut credit = PeakCredit::default();
        let mut values = UnorderedMap::<u64, u64>::new();
        {
            let mut record = graph.begin_record(0).unwrap();
            credit
                .map::<u64, u64>(&mut record.producer_scratch(), 4)
                .unwrap();
        }
        for n in 0..4 {
            values.insert(n, n);
        }
        assert_eq!(graph.charged_bytes(), first);
        assert_eq!(graph.charged_visits(), 1);
        let previous = credit.0;
        graph
            .charge_bytes(slug_build_api_v2::published_value::BYTE_LIMIT - first - growth + 1)
            .unwrap();
        let result = credit.map::<u64, u64>(&mut graph.producer_scratch(), 5);
        if result.is_ok() {
            values.insert(4, 4);
        }
        assert!(result.is_err());
        assert_eq!(values.len(), 4);
        assert_eq!(credit.0, previous);
        assert!(graph.charge_bytes(0).is_err());
    }
    #[test]
    fn compiled_event_first_allocation_precharges_exact_peak_and_refuses_one_under() {
        use slug_build_api_v2::published_value::BYTE_LIMIT;
        use starlark::eval::DefDependency;
        use starlark::values::Value;
        let required = 16 * std::mem::size_of::<DefDependency<'static>>()
            + std::mem::size_of::<Vec<DefDependency<'static>>>();
        for deficit in [0, 1] {
            let mut graph = PublishedValueArenaBuilder::new();
            graph.charge_bytes(BYTE_LIMIT - required + deficit).unwrap();
            let mut credit = PeakCredit::default();
            let mut events = Vec::<DefDependency<'static>>::new();
            let result = credit.vector::<DefDependency<'static>>(&mut graph.producer_scratch(), 1);
            if result.is_ok() {
                events.push(DefDependency::Constant(Value::new_none()));
            }
            assert_eq!(graph.charged_visits(), 0);
            if deficit == 0 {
                result.unwrap();
                assert_eq!(graph.charged_bytes(), BYTE_LIMIT);
                assert_eq!(credit.0, required);
                assert_eq!(events.len(), 1);
                assert!(events.capacity() >= events.len());
                assert!(matches!(&events[0], DefDependency::Constant(v) if v.is_none()));
            } else {
                assert!(result.is_err());
                assert_eq!(events.len(), 0);
                assert_eq!(events.capacity(), 0);
                assert_eq!(credit.0, 0);
                assert_eq!(graph.charged_bytes(), BYTE_LIMIT - required + 1);
                assert!(graph.charge_bytes(0).is_err());
            }
        }
    }

    #[test]
    fn compiled_event_later_growth_pays_only_delta_and_refuses_before_reallocation() {
        use slug_build_api_v2::published_value::BYTE_LIMIT;
        use starlark::eval::DefDependency;
        use starlark::values::Value;
        for deficit in [0, 1] {
            let mut graph = PublishedValueArenaBuilder::new();
            let mut credit = PeakCredit::default();
            let mut events = Vec::<DefDependency<'static>>::new();
            credit
                .vector::<DefDependency<'static>>(&mut graph.producer_scratch(), 1)
                .unwrap();
            events.push(DefDependency::Constant(Value::new_none()));
            let capacity = events.capacity();
            while events.len() < capacity {
                credit
                    .vector::<DefDependency<'static>>(
                        &mut graph.producer_scratch(),
                        events.len() + 1,
                    )
                    .unwrap();
                events.push(DefDependency::Constant(Value::new_none()));
            }
            let previous = credit.0;
            assert_eq!(graph.charged_bytes(), previous);
            let required =
                4 * (capacity + 1).max(4) * std::mem::size_of::<DefDependency<'static>>()
                    + std::mem::size_of::<Vec<DefDependency<'static>>>();
            let growth = required - previous;
            assert!(growth > 0);
            graph
                .charge_bytes(BYTE_LIMIT - previous - growth + deficit)
                .unwrap();
            let result = credit
                .vector::<DefDependency<'static>>(&mut graph.producer_scratch(), events.len() + 1);
            if result.is_ok() {
                events.push(DefDependency::Constant(Value::new_none()));
            }
            assert_eq!(graph.charged_visits(), 0);
            if deficit == 0 {
                result.unwrap();
                assert_eq!(graph.charged_bytes(), BYTE_LIMIT);
                assert_eq!(credit.0, required);
                assert_eq!(events.len(), capacity + 1);
                assert!(events.capacity() > capacity);
            } else {
                assert!(result.is_err());
                assert_eq!(events.len(), capacity);
                assert_eq!(events.capacity(), capacity);
                assert_eq!(credit.0, previous);
                assert_eq!(graph.charged_bytes(), BYTE_LIMIT - growth + 1);
                assert!(graph.charge_bytes(0).is_err());
            }
            assert!(
                events
                    .iter()
                    .all(|event| matches!(event, DefDependency::Constant(v) if v.is_none()))
            );
        }
    }
}
