//! Fixed request-local diagnostic counts; no source or semantic owner retention.
use super::selected_tool_source::ProgressOrigin;

#[derive(Clone, Copy, Default)]
pub(super) struct ProgressOrigins {
    counts: [u64; 5],
    overflow: bool,
}

impl ProgressOrigins {
    pub(super) fn record(&mut self, origin: ProgressOrigin) {
        let count = &mut self.counts[origin.code() - 1];
        if let Some(next) = count.checked_add(1) {
            *count = next;
        } else {
            self.overflow = true;
        }
    }

    pub(super) fn count(self, origin: ProgressOrigin) -> u64 {
        self.counts[origin.code() - 1]
    }

    pub(super) fn valid(self) -> bool {
        !self.overflow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_have_stable_distinct_numeric_codes_and_static_keys() {
        let expected = [
            (
                ProgressOrigin::InitialOwner,
                1,
                "progress_from_initial_owner",
            ),
            (
                ProgressOrigin::OwnerRefresh,
                2,
                "progress_from_owner_refresh",
            ),
            (ProgressOrigin::Plan, 3, "progress_from_plan"),
            (ProgressOrigin::DataPlan, 4, "progress_from_data_plan"),
            (ProgressOrigin::Sources, 5, "progress_from_sources"),
        ];
        for (index, (origin, code, key)) in expected.into_iter().enumerate() {
            assert_eq!(ProgressOrigin::ALL[index], origin);
            assert_eq!(origin.code(), code);
            assert_eq!(origin.receipt_key(), key);
        }
    }

    #[test]
    fn origin_counts_are_request_local_and_overflow_is_sticky_invalid() {
        assert!(std::mem::size_of::<ProgressOrigins>() <= 48);
        let mut first = ProgressOrigins::default();
        for origin in ProgressOrigin::ALL {
            first.record(origin);
            first.record(origin);
        }
        let second = ProgressOrigins::default();
        assert!(first.valid() && second.valid());
        for origin in ProgressOrigin::ALL {
            assert_eq!(first.count(origin), 2);
            assert_eq!(second.count(origin), 0);
        }
        first.counts[ProgressOrigin::Plan.code() - 1] = u64::MAX - 1;
        first.record(ProgressOrigin::Plan);
        assert!(first.valid());
        first.record(ProgressOrigin::Plan);
        first.record(ProgressOrigin::Sources);
        assert!(!first.valid());
        assert_eq!(first.count(ProgressOrigin::Plan), u64::MAX);
        assert_eq!(first.count(ProgressOrigin::Sources), 3);
        assert_eq!(second.count(ProgressOrigin::Plan), 0);
        assert!(second.valid());
    }
}
