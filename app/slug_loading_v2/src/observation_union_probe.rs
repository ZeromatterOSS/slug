//! Feature-only returned-helper diagnostics. No graph facts or observed payloads
//! are stored. Process-wide cells have no reset or request-isolation contract.
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

pub const HELPER_LABELS: [&str; 3] = ["host_bzl", "canonical_load_route", "selected_effect"];
pub const METRIC_LABELS: [&str; 10] = [
    "calls",
    "left_rows",
    "right_rows",
    "returned_rows",
    "successful_no_new_keys",
    "reused_left",
    "reused_right",
    "left_empty",
    "right_empty",
    "errors",
];

#[derive(Clone, Copy)]
#[repr(usize)]
pub(crate) enum Helper {
    HostBzl,
    CanonicalLoadRoute,
    SelectedEffect,
}

/// Only scalar properties of the successful returned epoch. Classes overlap:
/// e.g. an empty/empty union can reuse both owners, or neither owner.
pub(crate) struct Returned {
    pub(crate) rows: usize,
    pub(crate) reused_left: bool,
    pub(crate) reused_right: bool,
}

/// Relaxed per-cell observations, not an atomic cut or a request-isolated total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub counts: [[u64; 10]; 3],
    pub overflow_masks: [u64; 3],
}

struct Collector {
    counts: [[AtomicU64; 10]; 3],
    overflow_masks: [AtomicU64; 3],
}

impl Collector {
    const fn new() -> Self {
        Self {
            counts: [const { [const { AtomicU64::new(0) }; 10] }; 3],
            overflow_masks: [const { AtomicU64::new(0) }; 3],
        }
    }

    fn add(&self, helper: usize, metric: usize, amount: u64) {
        if amount == 0 {
            return;
        }
        let previous = self.counts[helper][metric]
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                Some(value.saturating_add(amount))
            })
            .expect("the saturating update always returns Some");
        if previous.checked_add(amount).is_none() {
            self.overflow_masks[helper].fetch_or(1 << metric, Ordering::Relaxed);
        }
    }

    fn rows(&self, helper: usize, metric: usize, rows: usize) {
        let amount = u64::try_from(rows).unwrap_or_else(|_| {
            self.overflow_masks[helper].fetch_or(1 << metric, Ordering::Relaxed);
            u64::MAX
        });
        self.add(helper, metric, amount);
    }

    fn record(&self, helper: Helper, left: usize, right: usize, returned: Option<Returned>) {
        let row = helper as usize;
        self.add(row, 0, 1);
        self.rows(row, 1, left);
        self.rows(row, 2, right);
        if let Some(returned) = returned {
            self.rows(row, 3, returned.rows);
            self.add(row, 4, u64::from(returned.rows == left));
            self.add(row, 5, u64::from(returned.reused_left));
            self.add(row, 6, u64::from(returned.reused_right));
        } else {
            self.add(row, 9, 1);
        }
        self.add(row, 7, u64::from(left == 0));
        self.add(row, 8, u64::from(right == 0));
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            counts: std::array::from_fn(|helper| {
                std::array::from_fn(|metric| self.counts[helper][metric].load(Ordering::Relaxed))
            }),
            overflow_masks: std::array::from_fn(|helper| {
                self.overflow_masks[helper].load(Ordering::Relaxed)
            }),
        }
    }
}

static COLLECTOR: Collector = Collector::new();

/// Record after the unchanged helper returns; callers supply only lengths and
/// O(1) owner-identity flags. No extra equality, subset scan or clock read.
pub(crate) fn record(helper: Helper, left: usize, right: usize, returned: Option<Returned>) {
    COLLECTOR.record(helper, left, right, returned);
}

pub fn snapshot() -> Snapshot {
    COLLECTOR.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returned_lengths_and_overlapping_classes_use_only_fixed_scalar_cells() {
        let collector = Collector::new();
        collector.record(
            Helper::HostBzl,
            3,
            1,
            Some(Returned {
                rows: 3,
                reused_left: true,
                reused_right: false,
            }),
        );
        collector.record(
            Helper::CanonicalLoadRoute,
            0,
            0,
            Some(Returned {
                rows: 0,
                reused_left: false,
                reused_right: false,
            }),
        );
        collector.record(Helper::SelectedEffect, 3, 2, None);
        let snapshot = collector.snapshot();
        assert_eq!(snapshot.counts[0], [1, 3, 1, 3, 1, 1, 0, 0, 0, 0]);
        assert_eq!(snapshot.counts[1], [1, 0, 0, 0, 1, 0, 0, 1, 1, 0]);
        assert_eq!(snapshot.counts[2], [1, 3, 2, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(snapshot.overflow_masks, [0; 3]);
        assert_eq!(std::mem::size_of::<Collector>(), 3 * 11 * 8);
        assert_eq!(HELPER_LABELS.len(), 3);
        assert_eq!(METRIC_LABELS.len(), 10);
    }

    #[test]
    fn additions_saturate_with_per_cell_masks_without_resetting_process_state() {
        let collector = Collector::new();
        for helper in 0..3 {
            for metric in 0..10 {
                collector.add(helper, metric, u64::MAX - 1);
                collector.add(helper, metric, 1);
                assert_eq!(
                    collector.snapshot().overflow_masks[helper],
                    (1 << metric) - 1
                );
                collector.add(helper, metric, 2);
                collector.add(helper, metric, 1);
            }
        }
        assert_eq!(collector.snapshot().counts, [[u64::MAX; 10]; 3]);
        assert_eq!(collector.snapshot().overflow_masks, [0x3ff; 3]);
    }

    #[test]
    fn concurrent_records_keep_complete_sums_after_join_without_retaining_inputs() {
        let collector = Collector::new();
        std::thread::scope(|threads| {
            for _ in 0..4 {
                threads.spawn(|| {
                    for _ in 0..100 {
                        collector.record(
                            Helper::SelectedEffect,
                            2,
                            1,
                            Some(Returned {
                                rows: 3,
                                reused_left: false,
                                reused_right: false,
                            }),
                        );
                    }
                });
            }
        });
        assert_eq!(
            collector.snapshot().counts[2],
            [400, 800, 400, 1200, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(collector.snapshot().overflow_masks, [0; 3]);
    }
}
