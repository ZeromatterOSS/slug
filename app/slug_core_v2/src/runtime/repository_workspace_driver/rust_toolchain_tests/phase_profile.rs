//! Fixed scalar diagnostics over the existing observer ABI, never graph facts.
use std::io;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use serde_json::Value;
use serde_json::json;
use slug_analysis_v2::ToolchainResolutionProbeSnapshot;
use slug_loading_v2::PathLookaheadProbeSnapshot;
use slug_loading_v2::RepositoryPackageChildProbeSnapshot;
use slug_loading_v2::observation_union_probe;
use slug_loading_v2::observation_union_probe::Snapshot as UnionSnapshot;

use crate::runtime::WorkspaceRuntime;
use crate::runtime::probe_observer::ProbeGuard;
use crate::runtime::probe_observer::family_counts::EVENT_LABELS;
use crate::runtime::probe_observer::family_counts::FAMILY_LABELS;
use crate::runtime::probe_observer::family_counts::Snapshot;

// The parent subtracts this reservation from its event trace allowance. The
// combined frontier and cleanup report still has the same 512 KiB ceiling.
pub(super) const REPORT_BYTES: usize = 16 * 1024;
const WORDS: usize = 64;
const MAGIC: u64 = u64::from_le_bytes(*b"SLGOBS01");

pub(super) struct InstalledProfile {
    guard: ProbeGuard,
    union_before: UnionSnapshot,
    path_before: PathLookaheadProbeSnapshot,
    toolchain_before: ToolchainResolutionProbeSnapshot,
    package_children_before: RepositoryPackageChildProbeSnapshot,
}
impl InstalledProfile {
    pub(super) fn attach(runtime: &mut WorkspaceRuntime) -> io::Result<Self> {
        let union_before = observation_union_probe::snapshot();
        let path_before = slug_loading_v2::path_lookahead_probe_snapshot();
        let toolchain_before = slug_analysis_v2::toolchain_resolution_probe_snapshot();
        let package_children_before = slug_loading_v2::repository_package_child_probe_snapshot();
        let raw = unsafe {
            libc::memfd_create(
                c"slug-rust-phase-profile".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        if unsafe { libc::ftruncate(fd.as_raw_fd(), 512) } != 0
            || unsafe {
                libc::fcntl(
                    fd.as_raw_fd(),
                    libc::F_ADD_SEALS,
                    libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL,
                )
            } != 0
        {
            return Err(io::Error::last_os_error());
        }
        let guard = ProbeGuard::install_from_fd(fd)?;
        runtime.attach_probe_observer(guard.observer());
        Ok(Self {
            guard,
            union_before,
            path_before,
            toolchain_before,
            package_children_before,
        })
    }
    /// Called only after the outcome and runtime have dropped and native owners
    /// have refunded. Disable first, then sample the still-owned atomic mapping.
    pub(super) fn finish(self) -> Value {
        let observer = self.guard.observer();
        drop(self.guard);
        let words = std::array::from_fn(|i| observer.words_for_test()[i].load(Ordering::Acquire));
        let counts = observer.family_counts_for_test();
        let mut profile = decode(&words);
        profile["scalar_counter_scope"] = scalar_counter_scope();
        profile["dice_family_counts"] = family_counts(counts);
        profile["observation_union_helpers"] =
            union_counts(self.union_before, observation_union_probe::snapshot());
        profile["metadata_path_lookahead"] = lookahead_counts(
            self.path_before,
            slug_loading_v2::path_lookahead_probe_snapshot(),
        );
        profile["toolchain_resolution_returns"] = toolchain_counts(
            self.toolchain_before,
            slug_analysis_v2::toolchain_resolution_probe_snapshot(),
        );
        profile["repository_package_child_needs"] = package_child_counts(
            self.package_children_before,
            slug_loading_v2::repository_package_child_probe_snapshot(),
        );
        profile
    }
}

fn scalar_counter_scope() -> Value {
    json!({
        "process_wide":true,"request_isolated":false,"atomic_snapshot":false,
        "per_cell_before_after":true,"reset_performed":false,
        "arithmetic_deltas_are_not_request_totals":true,
        "measures_time":false,"measures_allocation_count":false,
        "retains_keys_labels_paths_epochs_or_needs":false,
    })
}

fn union_counts(before: UnionSnapshot, after: UnionSnapshot) -> Value {
    let rows = observation_union_probe::HELPER_LABELS.iter().enumerate().map(|(index,label)| {
        let mut saturated_mask = 0u64;
        let mut reversed_mask = 0u64;
        let delta: [Option<u64>; 10] = std::array::from_fn(|metric| {
            let bit = 1 << metric;
            let initial = before.counts[index][metric];
            let final_count = after.counts[index][metric];
            if initial == u64::MAX || final_count == u64::MAX {
                saturated_mask |= bit;
            }
            let difference = final_count.checked_sub(initial);
            if difference.is_none() {
                reversed_mask |= bit;
            }
            if (before.overflow_masks[index] | after.overflow_masks[index] | saturated_mask) & bit != 0 {
                None
            } else {
                difference
            }
        });
        let overflow = before.overflow_masks[index] | after.overflow_masks[index];
        json!({
            "helper":label,"before_counts":before.counts[index],
            "after_counts":after.counts[index],"checked_delta":delta,
            "before_overflow_mask":before.overflow_masks[index],
            "after_overflow_mask":after.overflow_masks[index],
            "saturated_mask":saturated_mask,"reversed_mask":reversed_mask,
            "preexisting_overflow":before.overflow_masks[index]!=0,
            "unknown_overflow_bits":overflow&!0x3ff,
            "all_counter_deltas_arithmetically_exact":overflow==0&&saturated_mask==0&&reversed_mask==0,
        })
    }).collect::<Vec<_>>();
    json!({
        "metric_columns":observation_union_probe::METRIC_LABELS,"helpers":rows,
        "returned_helper_observations":true,"classes_overlap":true,
        "successful_no_new_keys_uses_union_cardinality":true,
        "scope_qualifications_from":"scalar_counter_scope",
    })
}

fn scalar_deltas<const N: usize>(
    before: [u64; N],
    after: [u64; N],
    before_mask: u64,
    after_mask: u64,
) -> ([Option<u64>; N], u64, u64, u64) {
    let mut saturated = 0u64;
    let mut reversed = 0u64;
    let overflow = before_mask | after_mask;
    let delta = std::array::from_fn(|index| {
        let bit = 1 << index;
        if before[index] == u64::MAX || after[index] == u64::MAX {
            saturated |= bit;
        }
        let difference = after[index].checked_sub(before[index]);
        if difference.is_none() {
            reversed |= bit;
        }
        if (overflow | saturated) & bit == 0 {
            difference
        } else {
            None
        }
    });
    (delta, saturated, reversed, overflow)
}

fn lookahead_counts(
    before: PathLookaheadProbeSnapshot,
    after: PathLookaheadProbeSnapshot,
) -> Value {
    let (delta, saturated, reversed, overflow) = scalar_deltas(
        before.counts,
        after.counts,
        before.overflow_mask,
        after.overflow_mask,
    );
    json!({
        "metric_columns":["eligible_windows_started","window_rows_inspected",
            "path_extensions_accepted","new_unique_demands_added"],
        "before_counts":before.counts,"after_counts":after.counts,"checked_delta":delta,
        "before_overflow_mask":before.overflow_mask,"after_overflow_mask":after.overflow_mask,
        "preexisting_overflow":before.overflow_mask!=0,"saturated_mask":saturated,
        "reversed_mask":reversed,"unknown_overflow_bits":overflow&!0xf,
        "all_counter_deltas_arithmetically_exact":overflow==0&&saturated==0&&reversed==0,
        "scope_qualifications_from":"scalar_counter_scope",
        "window_rows_include_complete_and_barrier_rows":true,
        "accepted_extensions_include_duplicate_only":true,
        "new_unique_demands_are_per_frontier_extension":true,
        "counts_distinct_global_demands":false,
    })
}

fn toolchain_counts(
    before: ToolchainResolutionProbeSnapshot,
    after: ToolchainResolutionProbeSnapshot,
) -> Value {
    let (delta, saturated, reversed, overflow) = scalar_deltas(
        before.counts,
        after.counts,
        before.overflow_mask,
        after.overflow_mask,
    );
    json!({
        "metric_columns":slug_analysis_v2::TOOLCHAIN_RESOLUTION_PROBE_LABELS,
        "before_counts":before.counts,"after_counts":after.counts,"checked_delta":delta,
        "before_overflow_mask":before.overflow_mask,"after_overflow_mask":after.overflow_mask,
        "preexisting_overflow":before.overflow_mask!=0,"saturated_mask":saturated,
        "reversed_mask":reversed,"unknown_overflow_bits":overflow&!0x1ffff,
        "all_counter_deltas_arithmetically_exact":overflow==0&&saturated==0&&reversed==0,
        "scope_qualifications_from":"scalar_counter_scope",
        "records_selected_parent_need_return":true,
        "nested_resolvers_count_independently":true,
        "candidate_registration_need_coverage_excludes_later_platform_loop":true,
        "entries_minus_needs_are_not_successes_or_cancellations":true,
        "abandonment_before_return_is_not_counted_as_need":true,
        "counts_distinct_keys":false,
    })
}

fn package_child_counts(
    before: RepositoryPackageChildProbeSnapshot,
    after: RepositoryPackageChildProbeSnapshot,
) -> Value {
    let (delta, saturated, reversed, overflow) = scalar_deltas(
        before.counts,
        after.counts,
        before.overflow_mask,
        after.overflow_mask,
    );
    json!({
        "metric_columns":slug_loading_v2::REPOSITORY_PACKAGE_CHILD_PROBE_LABELS,
        "before_counts":before.counts,"after_counts":after.counts,"checked_delta":delta,
        "before_overflow_mask":before.overflow_mask,"after_overflow_mask":after.overflow_mask,
        "saturated_mask":saturated,"reversed_mask":reversed,
        "unknown_overflow_bits":overflow&!0x1f,
        "all_counter_deltas_arithmetically_exact":overflow==0&&saturated==0&&reversed==0,
        "scope_qualifications_from":"scalar_counter_scope",
        "entries_minus_needs_are_not_successes_or_cancellations":true,
        "classifies_whole_singleton_path_need_only":true,
        "other_includes_mixed_nonpath_and_nonsingleton_needs":true,
        "new_unique_demands_are_per_frontier_extension":true,
        "counts_distinct_global_demands":false,
        "additions_are_not_confirmed_serviced":true,
    })
}

fn family_counts(snapshot: Snapshot) -> Value {
    json!({
        "event_columns":EVENT_LABELS,
        "report_encoding":"columnar_families_v1",
        "family_labels":FAMILY_LABELS,"counts":snapshot.counts,
        "overflow_masks":snapshot.overflow_masks,
        "observed_per_cell_counts":true,
        "sampled_after_runtime_retirement_and_guard_disable":true,
        "disable_is_atomic_snapshot_barrier":false,
        "already_entered_callbacks_may_finish_after_disable":true,
        "event_pairs_may_be_imbalanced":true,
        "finished_includes_cancellation":true,
        "synchronous_projections_emit_started_finished_only":true,
        "measures_self_time":false,"measures_success":false,
        "retains_full_key_identity":false,
    })
}

fn decode(words: &[u64; WORDS]) -> Value {
    // Existing scalar ABI V2: never decode phase/activity frames (words14..50).
    if words[0] != MAGIC || words[1] != 2 {
        return json!({"valid_abi":false});
    }
    let names = [
        "injection_revision",
        "root_compute",
        "legacy_need",
        "idle_drain",
    ];
    let intervals = names
        .iter()
        .enumerate()
        .map(|(category, name)| {
            let base = 51 + category * 3;
            let status = (words[63] >> (category * 12)) & 0xfff;
            json!({
                "kind":name,"closed_count":words[base],"wall_ns":words[base+1],
                "process_cpu_ns":words[base+2],"status":status,
                "missing_wall":status&1!=0,"missing_cpu":status&2!=0,
                "reversed_wall":status&4!=0,"reversed_cpu":status&8!=0,
                "count_overflow":status&16!=0,"wall_overflow":status&32!=0,
                "cpu_overflow":status&64!=0,"aborted":status&128!=0,
                "suppressed":status&256!=0,"unknown_status_bits":status&!0x1ff,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "valid_abi":true,"version":2,"disabled":words[3],
        "event_overflow":words[4],"sampled_event_dropped":words[5],
        "unknown_interval_status_bits":words[63]>>48,"intervals":intervals,
        "runtime_construction_measured":false,"cpu_includes_helpers":false,
        "closed_count_includes_aborted":true,
        "categories_are_exclusive":false,
    })
}

#[derive(Default, Clone, Copy)]
struct Bucket {
    count: u64,
    wall_ns: u64,
    max_wall_ns: u64,
}
#[derive(Default)]
pub(super) struct ProgressTiming {
    buckets: [Bucket; 7],
    pending: Option<(usize, Instant)>,
    pairing_mismatch: bool,
    incomplete: bool,
    clock_reversed: bool,
    overflow: bool,
}
impl ProgressTiming {
    pub(super) fn begin(&mut self, attempt: usize) {
        self.begin_at(attempt, Instant::now());
    }
    fn begin_at(&mut self, attempt: usize, start: Instant) {
        if self.pending.replace((attempt, start)).is_some() {
            self.pairing_mismatch = true;
            self.incomplete = true;
        }
    }
    pub(super) fn finish(&mut self, attempt: usize, kind: usize, end: Instant) {
        let Some((expected, start)) = self.pending.take() else {
            self.pairing_mismatch = true;
            self.incomplete = true;
            return;
        };
        if expected != attempt || kind >= self.buckets.len() {
            self.pairing_mismatch = true;
            self.incomplete = true;
            return;
        }
        let Some(duration) = end.checked_duration_since(start) else {
            self.clock_reversed = true;
            self.incomplete = true;
            return;
        };
        self.fold(kind, duration);
    }
    fn fold(&mut self, kind: usize, duration: Duration) {
        let bucket = &mut self.buckets[kind];
        let Ok(ns) = u64::try_from(duration.as_nanos()) else {
            self.overflow = true;
            return;
        };
        let Some((count, wall)) = bucket
            .count
            .checked_add(1)
            .zip(bucket.wall_ns.checked_add(ns))
        else {
            self.overflow = true;
            return;
        };
        bucket.count = count;
        bucket.wall_ns = wall;
        bucket.max_wall_ns = bucket.max_wall_ns.max(ns);
    }
    pub(super) fn summary(&self) -> Value {
        json!({
            "buckets":self.buckets.iter().enumerate().map(|(kind,b)|json!({
                "kind":super::progress_name(kind),"closed_count":b.count,
                "wall_ns":b.wall_ns,"max_wall_ns":b.max_wall_ns,
            })).collect::<Vec<_>>(),
            "pairing_mismatch":self.pairing_mismatch,"incomplete":self.incomplete||self.pending.is_some(),
            "clock_reversed":self.clock_reversed,"overflow":self.overflow,
            "includes_fixed_callback_dispatch_overhead":true,
            "overlaps_legacy_need":true,
        })
    }
}

#[test]
fn phase_profile_scalar_v2_qualifiers_and_progress_pairing_are_bounded() {
    let mut words = [0; WORDS];
    words[0] = MAGIC;
    words[1] = 2;
    words[3] = 1;
    words[4] = 4;
    words[5] = 3;
    words[54] = 2;
    words[55] = 23;
    words[56] = 7;
    words[63] = (128 | 32 | 2) << 12;
    words[14..51].fill(u64::MAX);
    let decoded = decode(&words);
    assert_eq!(decoded["intervals"][1]["closed_count"], 2);
    assert_eq!(decoded["intervals"][1]["wall_ns"], 23);
    assert_eq!(decoded["intervals"][1]["aborted"], true);
    assert_eq!(decoded["intervals"][1]["wall_overflow"], true);
    assert_eq!(decoded["intervals"][1]["missing_cpu"], true);
    assert_eq!(decoded["event_overflow"], 4);
    assert_eq!(decoded["sampled_event_dropped"], 3);
    assert!(!decoded.to_string().contains(&u64::MAX.to_string()));
    words[1] = 3;
    assert_eq!(decode(&words), json!({"valid_abi":false}));
    let start = Instant::now();
    let mut timing = ProgressTiming::default();
    timing.begin_at(1, start);
    timing.finish(1, 5, start + Duration::from_nanos(11));
    timing.begin_at(2, start);
    timing.finish(2, 6, start + Duration::from_nanos(3));
    assert_eq!(timing.summary()["buckets"][5]["wall_ns"], 11);
    assert_eq!(timing.summary()["buckets"][6]["closed_count"], 1);
    timing.begin_at(3, start);
    timing.finish(4, 0, start);
    assert!(timing.pairing_mismatch && timing.incomplete);
    timing.buckets[0].wall_ns = u64::MAX;
    timing.fold(0, Duration::from_nanos(1));
    assert!(timing.overflow);
    assert_eq!(timing.buckets[0].wall_ns, u64::MAX);
    timing.begin_at(5, start);
    assert_eq!(timing.summary()["incomplete"], true);
    assert!(decoded.to_string().len() + timing.summary().to_string().len() < REPORT_BYTES);
}

#[test]
fn phase_profile_family_report_is_static_and_bounded_at_maximum_counts() {
    use crate::runtime::probe_observer::family_counts::FAMILY_COUNT;

    let counts = family_counts(Snapshot {
        counts: [[u64::MAX; 6]; FAMILY_COUNT],
        overflow_masks: [u64::MAX; FAMILY_COUNT],
    });
    let labels = counts["family_labels"].as_array().unwrap();
    let rows = counts["counts"].as_array().unwrap();
    let masks = counts["overflow_masks"].as_array().unwrap();
    assert_eq!(counts["report_encoding"], "columnar_families_v1");
    assert_eq!(labels.len(), FAMILY_COUNT);
    assert_eq!(rows.len(), FAMILY_COUNT);
    assert_eq!(masks.len(), FAMILY_COUNT);
    for index in 0..FAMILY_COUNT {
        assert_eq!(labels[index], FAMILY_LABELS[index]);
        assert_eq!(rows[index], serde_json::to_value([u64::MAX; 6]).unwrap());
        assert_eq!(masks[index], u64::MAX);
    }
    assert_eq!(counts["event_columns"], json!(EVENT_LABELS));
    assert_eq!(counts["disable_is_atomic_snapshot_barrier"], false);
    assert_eq!(counts["event_pairs_may_be_imbalanced"], true);
    assert_eq!(counts["finished_includes_cancellation"], true);
    assert_eq!(counts["measures_self_time"], false);
    assert_eq!(counts["measures_success"], false);
    assert_eq!(counts["retains_full_key_identity"], false);

    // Only compile-time labels and scalar cells enter the encoder. There is no
    // observed key/tag, source path, argument or environment payload field.
    let encoded_counts = counts.to_string();
    for payload in [
        "arbitrary-key-payload",
        "/private/source",
        "secret-like-argv",
    ] {
        assert!(!encoded_counts.contains(payload));
    }
    let mut words = [u64::MAX; WORDS];
    words[0] = MAGIC;
    words[1] = 2;
    let mut profile = decode(&words);
    profile["scalar_counter_scope"] = scalar_counter_scope();
    profile["dice_family_counts"] = counts;
    profile["observation_union_helpers"] = union_counts(
        UnionSnapshot {
            counts: [[10_000_000_000_000_000_000; 10]; 3],
            overflow_masks: [0; 3],
        },
        UnionSnapshot {
            counts: [[u64::MAX - 1; 10]; 3],
            overflow_masks: [0; 3],
        },
    );
    profile["metadata_path_lookahead"] = lookahead_counts(
        PathLookaheadProbeSnapshot {
            counts: [10_000_000_000_000_000_000; 4],
            overflow_mask: 0,
        },
        PathLookaheadProbeSnapshot {
            counts: [u64::MAX - 1; 4],
            overflow_mask: 0,
        },
    );
    profile["toolchain_resolution_returns"] = toolchain_counts(
        ToolchainResolutionProbeSnapshot {
            counts: [10_000_000_000_000_000_000; 17],
            overflow_mask: 0,
        },
        ToolchainResolutionProbeSnapshot {
            counts: [u64::MAX - 1; 17],
            overflow_mask: 0,
        },
    );
    profile["repository_package_child_needs"] = package_child_counts(
        RepositoryPackageChildProbeSnapshot {
            counts: [10_000_000_000_000_000_000; 5],
            overflow_mask: 0,
        },
        RepositoryPackageChildProbeSnapshot {
            counts: [u64::MAX - 1; 5],
            overflow_mask: 0,
        },
    );
    for name in [
        "observation_union_helpers",
        "metadata_path_lookahead",
        "toolchain_resolution_returns",
        "repository_package_child_needs",
    ] {
        assert_eq!(
            profile[name]["scope_qualifications_from"],
            "scalar_counter_scope"
        );
    }
    for field in [
        "process_wide",
        "per_cell_before_after",
        "arithmetic_deltas_are_not_request_totals",
    ] {
        assert_eq!(profile["scalar_counter_scope"][field], true);
    }
    for field in [
        "request_isolated",
        "atomic_snapshot",
        "reset_performed",
        "measures_time",
        "measures_allocation_count",
        "retains_keys_labels_paths_epochs_or_needs",
    ] {
        assert_eq!(profile["scalar_counter_scope"][field], false);
    }
    let timing = ProgressTiming {
        buckets: [Bucket {
            count: u64::MAX,
            wall_ns: u64::MAX,
            max_wall_ns: u64::MAX,
        }; 7],
        pairing_mismatch: true,
        incomplete: true,
        clock_reversed: true,
        overflow: true,
        ..Default::default()
    };
    let combined = json!({"phase_profile":profile,"progress_wall":timing.summary()});
    // Long before/after cells and long exact deltas, with margin for the
    // mutually exclusive overflow form (which uses null deltas).
    assert!(
        combined.to_string().len() + 1024 < REPORT_BYTES,
        "combined report={} bytes; fixed reservation={REPORT_BYTES}",
        combined.to_string().len()
    );
    let mut masked_profile = combined["phase_profile"].clone();
    masked_profile["observation_union_helpers"] = union_counts(
        UnionSnapshot {
            counts: [[u64::MAX; 10]; 3],
            overflow_masks: [u64::MAX; 3],
        },
        UnionSnapshot {
            counts: [[u64::MAX; 10]; 3],
            overflow_masks: [u64::MAX; 3],
        },
    );
    masked_profile["metadata_path_lookahead"] = lookahead_counts(
        PathLookaheadProbeSnapshot {
            counts: [u64::MAX; 4],
            overflow_mask: u64::MAX,
        },
        PathLookaheadProbeSnapshot {
            counts: [u64::MAX; 4],
            overflow_mask: u64::MAX,
        },
    );
    masked_profile["toolchain_resolution_returns"] = toolchain_counts(
        ToolchainResolutionProbeSnapshot {
            counts: [u64::MAX; 17],
            overflow_mask: u64::MAX,
        },
        ToolchainResolutionProbeSnapshot {
            counts: [u64::MAX; 17],
            overflow_mask: u64::MAX,
        },
    );
    masked_profile["repository_package_child_needs"] = package_child_counts(
        RepositoryPackageChildProbeSnapshot {
            counts: [u64::MAX; 5],
            overflow_mask: u64::MAX,
        },
        RepositoryPackageChildProbeSnapshot {
            counts: [u64::MAX; 5],
            overflow_mask: u64::MAX,
        },
    );
    let masked = json!({"phase_profile":masked_profile,"progress_wall":timing.summary()});
    assert!(
        masked.to_string().len() + 1024 < REPORT_BYTES,
        "masked report={} bytes; fixed reservation={REPORT_BYTES}",
        masked.to_string().len()
    );
}

#[test]
fn union_helper_deltas_qualify_max_overflow_and_reversal_without_raw_inputs() {
    let mut before = UnionSnapshot {
        counts: [[0; 10]; 3],
        overflow_masks: [0; 3],
    };
    let mut after = before;
    before.counts[0][0] = 10;
    after.counts[0][0] = 15;
    before.counts[0][1] = 12;
    after.counts[0][1] = 11;
    before.counts[0][2] = u64::MAX;
    after.counts[0][2] = u64::MAX;
    before.overflow_masks[1] = 1;
    after.counts[1][0] = 2;
    after.overflow_masks[2] = 2;
    after.counts[2][1] = 3;
    let report = union_counts(before, after);
    assert_eq!(report["helpers"][0]["checked_delta"][0], 5);
    assert!(report["helpers"][0]["checked_delta"][1].is_null());
    assert!(report["helpers"][0]["checked_delta"][2].is_null());
    assert_eq!(report["helpers"][0]["reversed_mask"], 2);
    assert_eq!(report["helpers"][0]["saturated_mask"], 4);
    assert_eq!(report["helpers"][1]["before_overflow_mask"], 1);
    assert_eq!(report["helpers"][1]["preexisting_overflow"], true);
    assert!(report["helpers"][1]["checked_delta"][0].is_null());
    assert_eq!(report["helpers"][2]["after_overflow_mask"], 2);
    assert!(report["helpers"][2]["checked_delta"][1].is_null());
    for index in 0..3 {
        assert_eq!(
            report["helpers"][index]["all_counter_deltas_arithmetically_exact"],
            false
        );
        assert_eq!(
            report["helpers"][index]["helper"],
            observation_union_probe::HELPER_LABELS[index]
        );
    }
    assert_eq!(report["classes_overlap"], true);
    assert_eq!(report["scope_qualifications_from"], "scalar_counter_scope");
    assert_eq!(scalar_counter_scope()["request_isolated"], false);
    assert_eq!(scalar_counter_scope()["atomic_snapshot"], false);
    let clean = union_counts(
        UnionSnapshot {
            counts: [[0; 10]; 3],
            overflow_masks: [0; 3],
        },
        UnionSnapshot {
            counts: [[1; 10]; 3],
            overflow_masks: [0; 3],
        },
    );
    assert_eq!(
        clean["helpers"][0]["all_counter_deltas_arithmetically_exact"],
        true
    );
    assert_eq!(
        clean["helpers"][0]["checked_delta"],
        serde_json::to_value([1u64; 10]).unwrap()
    );
    for payload in [
        "arbitrary-key-payload",
        "/private/source",
        "secret-like-argv",
    ] {
        assert!(!report.to_string().contains(payload));
    }
}

#[test]
fn metadata_path_probe_deltas_preserve_overflow_and_process_scope_qualifications() {
    let report = lookahead_counts(
        PathLookaheadProbeSnapshot {
            counts: [10, 12, u64::MAX, 1],
            overflow_mask: 8,
        },
        PathLookaheadProbeSnapshot {
            counts: [15, 11, u64::MAX, 2],
            overflow_mask: 0,
        },
    );
    assert_eq!(report["checked_delta"], json!([5, null, null, null]));
    assert_eq!(report["saturated_mask"], 4);
    assert_eq!(report["reversed_mask"], 2);
    assert_eq!(report["preexisting_overflow"], true);
    assert_eq!(report["all_counter_deltas_arithmetically_exact"], false);
    assert_eq!(report["scope_qualifications_from"], "scalar_counter_scope");
    let clean = lookahead_counts(
        PathLookaheadProbeSnapshot {
            counts: [0; 4],
            overflow_mask: 0,
        },
        PathLookaheadProbeSnapshot {
            counts: [1; 4],
            overflow_mask: 0,
        },
    );
    assert_eq!(
        clean["checked_delta"],
        serde_json::to_value([1u64; 4]).unwrap()
    );
    assert_eq!(clean["all_counter_deltas_arithmetically_exact"], true);
}

#[test]
fn toolchain_stage_probe_preserves_masks_and_return_only_qualifications() {
    let mut before = ToolchainResolutionProbeSnapshot {
        counts: [10; 17],
        overflow_mask: 8,
    };
    let mut after = ToolchainResolutionProbeSnapshot {
        counts: [15; 17],
        overflow_mask: 1 << 63,
    };
    after.counts[1] = 9;
    before.counts[2] = u64::MAX;
    after.counts[2] = u64::MAX;
    let report = toolchain_counts(before, after);
    assert_eq!(report["checked_delta"][0], 5);
    assert!(report["checked_delta"][1].is_null());
    assert!(report["checked_delta"][2].is_null());
    assert!(report["checked_delta"][3].is_null());
    assert_eq!(report["checked_delta"][4], 5);
    assert_eq!(report["saturated_mask"], 4);
    assert_eq!(report["reversed_mask"], 2);
    assert_eq!(report["unknown_overflow_bits"], 1u64 << 63);
    assert_eq!(report["all_counter_deltas_arithmetically_exact"], false);
    assert_eq!(report["metric_columns"].as_array().unwrap().len(), 17);
    assert_eq!(report["scope_qualifications_from"], "scalar_counter_scope");
    for field in [
        "records_selected_parent_need_return",
        "nested_resolvers_count_independently",
        "candidate_registration_need_coverage_excludes_later_platform_loop",
        "entries_minus_needs_are_not_successes_or_cancellations",
        "abandonment_before_return_is_not_counted_as_need",
    ] {
        assert_eq!(report[field], true);
    }
    for field in ["counts_distinct_keys"] {
        assert_eq!(report[field], false);
    }
    let clean = toolchain_counts(
        ToolchainResolutionProbeSnapshot {
            counts: [0; 17],
            overflow_mask: 0,
        },
        ToolchainResolutionProbeSnapshot {
            counts: [1; 17],
            overflow_mask: 0,
        },
    );
    assert_eq!(
        clean["checked_delta"],
        serde_json::to_value([1u64; 17]).unwrap()
    );
    assert_eq!(clean["all_counter_deltas_arithmetically_exact"], true);
}

#[test]
fn package_child_probe_report_keeps_masks_and_whole_need_qualification() {
    let report = package_child_counts(
        RepositoryPackageChildProbeSnapshot {
            counts: [10, 12, u64::MAX, 1, 5],
            overflow_mask: 8,
        },
        RepositoryPackageChildProbeSnapshot {
            counts: [15, 11, u64::MAX, 2, 8],
            overflow_mask: 1 << 63,
        },
    );
    assert_eq!(report["checked_delta"], json!([5, null, null, null, 3]));
    assert_eq!(report["saturated_mask"], 4);
    assert_eq!(report["reversed_mask"], 2);
    assert_eq!(report["unknown_overflow_bits"], 1u64 << 63);
    assert_eq!(report["all_counter_deltas_arithmetically_exact"], false);
    assert_eq!(report["scope_qualifications_from"], "scalar_counter_scope");
    assert_eq!(report["classifies_whole_singleton_path_need_only"], true);
    assert_eq!(
        report["other_includes_mixed_nonpath_and_nonsingleton_needs"],
        true
    );
    assert_eq!(
        report["new_unique_demands_are_per_frontier_extension"],
        true
    );
    assert_eq!(report["counts_distinct_global_demands"], false);
    assert_eq!(report["additions_are_not_confirmed_serviced"], true);
    let clean = package_child_counts(
        RepositoryPackageChildProbeSnapshot {
            counts: [0; 5],
            overflow_mask: 0,
        },
        RepositoryPackageChildProbeSnapshot {
            counts: [1; 5],
            overflow_mask: 0,
        },
    );
    assert_eq!(
        clean["checked_delta"],
        serde_json::to_value([1u64; 5]).unwrap()
    );
    assert_eq!(clean["all_counter_deltas_arithmetically_exact"], true);
}
