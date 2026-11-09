//! Feature-only returned-parent Need attribution. These process-wide cells
//! retain no keys, labels from inputs, Need values, clocks or graph ownership.
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

pub const TOOLCHAIN_RESOLUTION_PROBE_LABELS: [&str; 17] = [
    "entered",
    "target_platform_need",
    "execution_platform_registrations_need",
    "full_registrations_need",
    "candidate_platforms_need",
    "requested_constraints_need",
    "preferred_platform_need",
    "initial_package_closure_need",
    "required_types_need",
    "declaration_targets_need",
    "actual_declaration_package_closure_need",
    "declaration_types_need",
    "target_settings_need",
    "declaration_constraints_need",
    "candidate_preparation_entered",
    "candidate_full_registrations_need",
    "candidate_execution_only_registrations_need",
];

#[repr(usize)]
pub(super) enum NeedStage {
    TargetPlatform = 1,
    ExecutionPlatformRegistrations,
    FullRegistrations,
    CandidatePlatforms,
    RequestedConstraints,
    PreferredPlatform,
    InitialPackageClosure,
    RequiredTypes,
    DeclarationTargets,
    ActualDeclarationPackageClosure,
    DeclarationTypes,
    TargetSettings,
    DeclarationConstraints,
    CandidateFullRegistrations = 15,
    CandidateExecutionOnlyRegistrations,
}

/// Relaxed per-cell observations, not an atomic snapshot or request-isolated
/// totals. Entered includes calls later cancelled; Need cells count only actual
/// parent returns, never abandoned child Needs, completion or successful work.
/// Candidate preparation follows joined resolver reduction. Its platform-loop
/// Needs are not counted, so entry minus its two Need cells is not completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolchainResolutionProbeSnapshot {
    pub counts: [u64; 17],
    pub overflow_mask: u64,
}

struct Counters {
    counts: [AtomicU64; 17],
    overflow: AtomicU64,
}
impl Counters {
    const fn new() -> Self {
        Self {
            counts: [const { AtomicU64::new(0) }; 17],
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

    fn snapshot(&self) -> ToolchainResolutionProbeSnapshot {
        ToolchainResolutionProbeSnapshot {
            counts: std::array::from_fn(|i| self.counts[i].load(Ordering::Relaxed)),
            overflow_mask: self.overflow.load(Ordering::Relaxed),
        }
    }
}

static COUNTERS: Counters = Counters::new();

pub(super) fn entered() {
    COUNTERS.add(0, 1);
}

pub(super) fn returned_need(stage: NeedStage) {
    COUNTERS.add(stage as usize, 1);
}

pub(super) fn candidate_preparation_entered() {
    COUNTERS.add(14, 1);
}

pub fn toolchain_resolution_probe_snapshot() -> ToolchainResolutionProbeSnapshot {
    COUNTERS.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_stage_cells_saturate_with_complete_masks_without_global_reset() {
        let counts = Counters::new();
        assert_eq!(std::mem::size_of::<Counters>(), 144);
        assert_eq!(TOOLCHAIN_RESOLUTION_PROBE_LABELS.len(), 17);
        for metric in 0..17 {
            counts.add(metric, u64::MAX - 1);
            counts.add(metric, 1);
            assert_eq!(counts.snapshot().overflow_mask, (1 << metric) - 1);
            counts.add(metric, 2);
            counts.add(metric, 1);
        }
        assert_eq!(counts.snapshot().counts, [u64::MAX; 17]);
        assert_eq!(counts.snapshot().overflow_mask, 0x1ffff);
    }

    #[test]
    fn concurrent_local_stage_counts_keep_all_updates_without_payload_storage() {
        let counts = Counters::new();
        std::thread::scope(|threads| {
            for _ in 0..4 {
                threads.spawn(|| {
                    for _ in 0..100 {
                        for metric in 0..17 {
                            counts.add(metric, metric as u64 + 1);
                        }
                    }
                });
            }
        });
        for metric in 0..17 {
            assert_eq!(counts.snapshot().counts[metric], 400 * (metric as u64 + 1));
        }
        assert_eq!(counts.snapshot().overflow_mask, 0);
    }
}
