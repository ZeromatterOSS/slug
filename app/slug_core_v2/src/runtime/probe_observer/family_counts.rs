//! Fixed test-only DICE activity attribution. Observed tags are borrowed only
//! for classification; neither the collector nor its snapshot retains them.
//! These counts carry no timing, successful-computation or atomic-drain claim.

use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

pub(crate) const EVENT_LABELS: [&str; 6] = [
    "started",
    "finished",
    "check_deps_started",
    "check_deps_finished",
    "compute_started",
    "compute_finished",
];

pub(crate) const FAMILY_LABELS: [&str; 25] = [
    "command_configuration",
    "selected_effect",
    "selected_effect_observed",
    "canonical_route",
    "bzl_module",
    "package_load",
    "package_inventory",
    "configured_analysis",
    "captured_source_snapshot",
    "materialization_result",
    "materialization_request",
    "workspace_context_projection",
    "workspace_transcript_projection",
    "source_snapshot_projection",
    "materialization_projection",
    "path_observation",
    "path_input",
    "resolved_path",
    "anchored_resolved_path",
    "file_digest",
    "source_artifact_input",
    "root_repository_mapping",
    "selected_owner_certificate",
    "repository_package_source",
    "other",
];

pub(crate) const FAMILY_COUNT: usize = FAMILY_LABELS.len();
const OTHER: usize = FAMILY_COUNT - 1;

/// Relaxed per-cell observations. Callback entry/finish pairs may be imbalanced,
/// including across disable: callbacks already through the observer gate can
/// still finish. ComputeFinished also runs on cancellation, not only success.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Snapshot {
    pub(crate) counts: [[u64; 6]; FAMILY_COUNT],
    pub(crate) overflow_masks: [u64; FAMILY_COUNT],
}

pub(super) struct FamilyCounts {
    counts: [[AtomicU64; 6]; FAMILY_COUNT],
    overflow_masks: [AtomicU64; FAMILY_COUNT],
}

pub(crate) const STORAGE_BYTES: usize = std::mem::size_of::<FamilyCounts>();

impl Default for FamilyCounts {
    fn default() -> Self {
        Self {
            counts: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU64::new(0))),
            overflow_masks: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl FamilyCounts {
    /// Called only for the six exhaustive DiceEvent variants, after the existing
    /// observer entry gate and before the existing sampling early return.
    pub(super) fn record(&self, event: usize, tag: &str) {
        let family = classify(tag);
        if self.counts[family][event]
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                count.checked_add(1)
            })
            .is_err()
        {
            self.overflow_masks[family].fetch_or(1 << event, Ordering::Relaxed);
        }
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            counts: std::array::from_fn(|family| {
                std::array::from_fn(|event| self.counts[family][event].load(Ordering::Relaxed))
            }),
            overflow_masks: std::array::from_fn(|family| {
                self.overflow_masks[family].load(Ordering::Relaxed)
            }),
        }
    }
}

// DICE Key/ProjectionKey::key_type_name supplies the short static type name.
// Exact type matches keep changed/unknown types visible in `other`; no caller
// identity, substring matching or dynamic label enters the report.
fn classify(tag: &str) -> usize {
    match tag {
        "CommandConfigurationPreparationKey" | "CommandConfigurationPreparationObservationKey" => 0,
        "HostSelectedRepositoryFileEffectKey" => 1,
        "HostSelectedRepositoryFileEffectObservationKey" => 2,
        "HostCanonicalRepositoryLoadRouteKey"
        | "HostCanonicalRepositoryLoadRouteObservationKey" => 3,
        "HostBzlModuleEvalKey"
        | "HostBzlModuleObservationKey"
        | "ExternalBzlModuleEvalKey"
        | "ExternalBzlModuleObservationKey"
        | "BzlModuleEvalKey" => 4,
        "RootPackageLoadKey"
        | "RootPackageLoadObservationKey"
        | "RepositoryPackageLoadKey"
        | "RepositoryPackageLoadObservationKey" => 5,
        "RepositoryPackageInventoryKey" | "RepositoryPackageInventoryObservationKey" => 6,
        "ConfiguredNodeAnalysisKey" | "ConfiguredNodeAnalysisObservationKey" => 7,
        "CapturedSourceSnapshotKey" => 8,
        "RepositoryMaterializationResultKey" => 9,
        "RepositoryMaterializationRequestKey"
        | "RepositoryMaterializationRequestObservationKey" => 10,
        "RepositoryWorkspaceContextProjection" => 11,
        "RepositoryWorkspaceTranscriptProjection" => 12,
        "RepositorySourceSnapshotProjection" => 13,
        "RepositoryMaterializationProjection"
        | "RepositoryMaterializationResultEntryProjection" => 14,
        "PathObservationKey" => 15,
        "PathObservationInputKey"
        | "PathObservationInputModeKey"
        | "PathObservationInputModeProjectionKey" => 16,
        "ResolvedPathKey" | "ResolvedPathObservationKey" => 17,
        "AnchoredResolvedPathKey" | "AnchoredResolvedPathObservationKey" => 18,
        "PathFileDigestKey"
        | "PathFileDigestObservationKey"
        | "AnchoredPathFileDigestObservationKey" => 19,
        "SourceArtifactInputObservationKey" => 20,
        "HostRootRepositoryMappingKey" | "HostRootRepositoryMappingObservationKey" => 21,
        "HostSelectedExtensionOwnerCertificateKey"
        | "HostSelectedExtensionOwnerCertificateObservationKey" => 22,
        "RepositoryPackageSourceKey"
        | "RepositoryPackageSourceObservationKey"
        | "RootPackageSourceKey"
        | "RootPackageSourceObservationKey" => 23,
        _ => OTHER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_families_and_all_six_events_keep_unknown_tags_out_of_snapshot() {
        let collector = FamilyCounts::default();
        let examples = [
            "CommandConfigurationPreparationObservationKey",
            "HostSelectedRepositoryFileEffectKey",
            "HostSelectedRepositoryFileEffectObservationKey",
            "HostCanonicalRepositoryLoadRouteObservationKey",
            "ExternalBzlModuleObservationKey",
            "RepositoryPackageLoadObservationKey",
            "RepositoryPackageInventoryObservationKey",
            "ConfiguredNodeAnalysisObservationKey",
            "CapturedSourceSnapshotKey",
            "RepositoryMaterializationResultKey",
            "RepositoryMaterializationRequestObservationKey",
            "RepositoryWorkspaceContextProjection",
            "RepositoryWorkspaceTranscriptProjection",
            "RepositorySourceSnapshotProjection",
            "RepositoryMaterializationProjection",
            "PathObservationKey",
            "PathObservationInputModeProjectionKey",
            "ResolvedPathObservationKey",
            "AnchoredResolvedPathObservationKey",
            "AnchoredPathFileDigestObservationKey",
            "SourceArtifactInputObservationKey",
            "HostRootRepositoryMappingObservationKey",
            "HostSelectedExtensionOwnerCertificateObservationKey",
            "RepositoryPackageSourceObservationKey",
            "credential=/private/input?token=not-retained",
        ];
        assert_eq!(examples.len(), FAMILY_COUNT);
        for (family, tag) in examples.iter().enumerate() {
            assert_eq!(classify(tag), family);
            for event in 0..EVENT_LABELS.len() {
                collector.record(event, tag);
            }
        }
        let snapshot = collector.snapshot();
        assert_eq!(snapshot.counts, [[1; 6]; FAMILY_COUNT]);
        assert_eq!(snapshot.overflow_masks, [0; FAMILY_COUNT]);
        let rendered = format!("{snapshot:?} {FAMILY_LABELS:?} {EVENT_LABELS:?}");
        assert!(!rendered.contains("credential"));
        assert!(!rendered.contains("/private"));
        assert!(!rendered.contains("not-retained"));
        // Near matches never acquire the known classification.
        assert_eq!(classify("CapturedSourceSnapshotKey-extra"), OTHER);
        assert_eq!(classify("prefix::CapturedSourceSnapshotKey"), OTHER);
    }

    #[test]
    fn saturation_is_per_event_and_never_wraps_or_changes_other_cells() {
        let collector = FamilyCounts::default();
        for event in 0..6 {
            collector.counts[8][event].store(u64::MAX - 1, Ordering::Relaxed);
            collector.record(event, "CapturedSourceSnapshotKey");
            assert_eq!(collector.snapshot().overflow_masks[8], (1 << event) - 1);
            collector.record(event, "CapturedSourceSnapshotKey");
            collector.record(event, "CapturedSourceSnapshotKey");
        }
        collector.record(0, "Unknown");
        let snapshot = collector.snapshot();
        assert_eq!(snapshot.counts[8], [u64::MAX; 6]);
        assert_eq!(snapshot.overflow_masks[8], 0b11_1111);
        assert_eq!(snapshot.counts[OTHER], [1, 0, 0, 0, 0, 0]);
        assert_eq!(snapshot.overflow_masks[OTHER], 0);
        assert_eq!(snapshot.counts[0], [0; 6]);
    }

    #[test]
    fn concurrent_counts_include_every_event_without_sampling() {
        let collector = FamilyCounts::default();
        std::thread::scope(|threads| {
            for _ in 0..4 {
                threads.spawn(|| {
                    for _ in 0..1025 {
                        for event in 0..6 {
                            collector.record(event, "RepositoryWorkspaceTranscriptProjection");
                            collector.record(event, "Unknown");
                        }
                    }
                });
            }
        });
        let snapshot = collector.snapshot();
        assert_eq!(snapshot.counts[12], [4100; 6]);
        assert_eq!(snapshot.counts[OTHER], [4100; 6]);
        assert_eq!(snapshot.overflow_masks, [0; FAMILY_COUNT]);
    }
}
