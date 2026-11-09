//! Feature-only observations of the package BUILD-child owner's final Needs.
//! These process-wide cells are not atomic as a group or request-isolated.
//! They retain no input, path, Need, clock, or graph ownership.
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use slug_bzlmod_v2::SourcePreparationNeeds;
use slug_workspace_v2::PathObservationOperation;

pub const REPOSITORY_PACKAGE_CHILD_PROBE_LABELS: [&str; 5] = [
    "entered",
    "singleton_lstat_need",
    "singleton_file_bytes_need",
    "other_need",
    "lookahead_unique_demands_added_per_frontier",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepositoryPackageChildProbeSnapshot {
    pub counts: [u64; 5],
    pub overflow_mask: u64,
}

struct Counters {
    counts: [AtomicU64; 5],
    overflow: AtomicU64,
}

impl Counters {
    const fn new() -> Self {
        Self {
            counts: [const { AtomicU64::new(0) }; 5],
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

    fn snapshot(&self) -> RepositoryPackageChildProbeSnapshot {
        RepositoryPackageChildProbeSnapshot {
            counts: std::array::from_fn(|i| self.counts[i].load(Ordering::Relaxed)),
            overflow_mask: self.overflow.load(Ordering::Relaxed),
        }
    }
}

fn need_cell(need: &SourcePreparationNeeds) -> usize {
    match need.only_path_observations() {
        Some(paths) if paths.demands().len() == 1 => match paths.demands()[0].operation() {
            PathObservationOperation::Lstat => 1,
            PathObservationOperation::FileBytes => 2,
            _ => 3,
        },
        _ => 3,
    }
}

static COUNTERS: Counters = Counters::new();

pub(super) fn entered() {
    COUNTERS.add(0, 1);
}

pub(super) fn returned_need(need: &SourcePreparationNeeds) {
    COUNTERS.add(need_cell(need), 1);
}

pub(super) fn lookahead_extended(added: usize) {
    COUNTERS.add(4, added as u64);
}

pub fn repository_package_child_probe_snapshot() -> RepositoryPackageChildProbeSnapshot {
    COUNTERS.snapshot()
}

#[cfg(test)]
mod tests {
    use compact_str::CompactString;
    use slug_bzlmod_v2::NeedRepositoryEnvironmentNames;
    use slug_bzlmod_v2::RepositoryEnvironmentNameFrontier;
    use slug_workspace_v2::NeedPathObservations;
    use slug_workspace_v2::NormalizedAbsolutePath;
    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationNamespace;

    use super::*;

    fn need(
        operations: impl IntoIterator<Item = PathObservationOperation>,
    ) -> SourcePreparationNeeds {
        SourcePreparationNeeds::path(
            NeedPathObservations::try_from_iter(operations.into_iter().enumerate().map(
                |(i, op)| {
                    PathObservationDemand::new(
                        PathObservationNamespace::Host,
                        NormalizedAbsolutePath::new(format!("/probe/{i}")).unwrap(),
                        op,
                    )
                },
            ))
            .unwrap(),
        )
    }

    #[test]
    fn whole_need_classification_keeps_mixed_and_nonsingleton_needs_other() {
        assert_eq!(need_cell(&need([PathObservationOperation::Lstat])), 1);
        assert_eq!(need_cell(&need([PathObservationOperation::FileBytes])), 2);
        for operations in [
            vec![PathObservationOperation::FileDigest],
            vec![PathObservationOperation::ReadLink],
            vec![
                PathObservationOperation::Lstat,
                PathObservationOperation::Lstat,
            ],
            vec![
                PathObservationOperation::Lstat,
                PathObservationOperation::FileBytes,
            ],
        ] {
            assert_eq!(need_cell(&need(operations)), 3);
        }
        let environment = SourcePreparationNeeds::environment(
            NeedRepositoryEnvironmentNames::new(
                NormalizedAbsolutePath::new("/probe").unwrap(),
                RepositoryEnvironmentNameFrontier::from_unsorted([CompactString::new("ENV")]),
            )
            .unwrap(),
        );
        assert_eq!(need_cell(&environment), 3);
        let mixed = need([PathObservationOperation::Lstat])
            .try_union(&environment)
            .unwrap();
        assert_eq!(need_cell(&mixed), 3);
    }

    #[test]
    fn local_counter_layout_saturation_and_concurrency_preserve_all_cells() {
        let counts = Counters::new();
        assert_eq!(std::mem::size_of::<Counters>(), 48);
        assert_eq!(REPOSITORY_PACKAGE_CHILD_PROBE_LABELS.len(), 5);
        std::thread::scope(|threads| {
            for _ in 0..4 {
                threads.spawn(|| {
                    for _ in 0..100 {
                        for metric in 0..5 {
                            counts.add(metric, metric as u64 + 1);
                        }
                    }
                });
            }
        });
        for metric in 0..5 {
            let value = 400 * (metric as u64 + 1);
            assert_eq!(counts.snapshot().counts[metric], value);
            counts.add(metric, u64::MAX - value);
            assert_eq!(counts.snapshot().overflow_mask, (1 << metric) - 1);
            counts.add(metric, 1);
        }
        assert_eq!(counts.snapshot().counts, [u64::MAX; 5]);
        assert_eq!(counts.snapshot().overflow_mask, 31);
    }
}
