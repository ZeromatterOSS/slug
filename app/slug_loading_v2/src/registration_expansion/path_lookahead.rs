//! Registration diagnostics for the shared bounded Lstat lookahead policy.
use super::DriverBreak;
use crate::path_need_lookahead::PathNeedBreak;

impl PathNeedBreak for DriverBreak {
    fn into_need(self) -> Result<slug_bzlmod_v2::SourcePreparationNeeds, Self> {
        match self {
            Self::Need(need) => Ok(need),
            other => Err(other),
        }
    }
    fn from_need(need: slug_bzlmod_v2::SourcePreparationNeeds) -> Self {
        Self::Need(need)
    }
    #[cfg(feature = "native-probe-observer")]
    fn lookahead_started() {
        probe::started();
    }
    #[cfg(feature = "native-probe-observer")]
    fn lookahead_inspected() {
        probe::inspected();
    }
    #[cfg(feature = "native-probe-observer")]
    fn lookahead_extended(added: usize) {
        probe::extended(added);
    }
}

/// Fixed process-wide reachability cells, never request-isolated or atomic as
/// a group. The feature does not affect any eligibility or scheduling decision.
#[cfg(feature = "native-probe-observer")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathLookaheadProbeSnapshot {
    pub counts: [u64; 4],
    pub overflow_mask: u64,
}

#[cfg(feature = "native-probe-observer")]
pub fn path_lookahead_probe_snapshot() -> PathLookaheadProbeSnapshot {
    probe::snapshot()
}

#[cfg(feature = "native-probe-observer")]
mod probe {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;

    use super::PathLookaheadProbeSnapshot;

    struct Counters {
        counts: [AtomicU64; 4],
        overflow: AtomicU64,
    }
    impl Counters {
        const fn new() -> Self {
            Self {
                counts: [const { AtomicU64::new(0) }; 4],
                overflow: AtomicU64::new(0),
            }
        }
        fn add(&self, metric: usize, amount: u64) {
            if amount == 0 {
                return;
            }
            let previous = self.counts[metric]
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    Some(value.saturating_add(amount))
                })
                .expect("the saturating update always returns Some");
            if previous.checked_add(amount).is_none() {
                self.overflow.fetch_or(1 << metric, Ordering::Relaxed);
            }
        }
        fn snapshot(&self) -> PathLookaheadProbeSnapshot {
            PathLookaheadProbeSnapshot {
                counts: std::array::from_fn(|i| self.counts[i].load(Ordering::Relaxed)),
                overflow_mask: self.overflow.load(Ordering::Relaxed),
            }
        }
    }
    static COUNTERS: Counters = Counters::new();

    pub(super) fn started() {
        COUNTERS.add(0, 1);
        COUNTERS.add(1, 1);
    }
    pub(super) fn inspected() {
        COUNTERS.add(1, 1);
    }
    pub(super) fn extended(added: usize) {
        COUNTERS.add(2, 1);
        // The accepted unique-demand window is at most32 on every target.
        COUNTERS.add(3, added as u64);
    }
    pub(super) fn snapshot() -> PathLookaheadProbeSnapshot {
        COUNTERS.snapshot()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn local_counter_saturates_without_a_process_reset() {
            let counts = Counters::new();
            assert_eq!(std::mem::size_of::<Counters>(), 40);
            for metric in 0..4 {
                counts.add(metric, u64::MAX - 1);
                counts.add(metric, 1);
                assert_eq!(counts.snapshot().overflow_mask, (1 << metric) - 1);
                counts.add(metric, 2);
                counts.add(metric, 1);
            }
            assert_eq!(counts.snapshot().counts, [u64::MAX; 4]);
            assert_eq!(counts.snapshot().overflow_mask, 15);
        }

        #[test]
        fn local_concurrent_counts_preserve_all_scalar_updates() {
            let counts = Counters::new();
            std::thread::scope(|threads| {
                for _ in 0..4 {
                    threads.spawn(|| {
                        for _ in 0..100 {
                            for metric in 0..4 {
                                counts.add(metric, metric as u64 + 1);
                            }
                        }
                    });
                }
            });
            assert_eq!(counts.snapshot().counts, [400, 800, 1200, 1600]);
            assert_eq!(counts.snapshot().overflow_mask, 0);
        }
    }
}
