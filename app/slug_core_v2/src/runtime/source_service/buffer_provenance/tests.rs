use std::io::Write;
use std::sync::Arc;
use std::sync::Barrier;
use std::time::Duration;
use std::time::Instant;

use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::dice::source_service::RegistryRequestAssociation;
use crate::runtime::dice::source_service::SourceServiceError;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::dice::source_service::SourceServiceScope;

fn scope(limits: SourceServiceLimits) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/buffer-provenance-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}
fn armed() -> (Arc<SourceServiceScope>, Arc<RegistryRequestAssociation>) {
    let owner = scope(SourceServiceLimits::default());
    let recorder = owner.arm_registry_association_for_test().unwrap();
    (owner, recorder)
}
fn origin(recorder: &RegistryRequestAssociation) -> SourceBufferOrigin {
    let snapshot = recorder.snapshot().buffer;
    assert!(snapshot.armed && snapshot.first_error_is_buffer_limit);
    snapshot.origin.unwrap()
}

#[test]
fn exact_reservation_ceiling_records_first_request_before_sticky_and_release() {
    let (owner, recorder) = armed();
    let baseline = owner.snapshot().live_buffer_bytes;
    let limit = owner.limits().live_buffer_bytes;
    let held = owner
        .reserve_buffer_at(limit - baseline, SourceBufferSite::SelectedPayload)
        .unwrap();
    assert_eq!(owner.snapshot().peak_buffer_bytes, limit);
    assert!(recorder.snapshot().buffer.origin.is_none());
    assert_eq!(
        owner
            .reserve_buffer_at(1, SourceBufferSite::SelectedScratch)
            .err(),
        Some(SourceServiceError::BufferLimit)
    );
    let first = origin(&recorder);
    assert_eq!(
        first,
        SourceBufferOrigin {
            site: SourceBufferSite::SelectedScratch,
            kind: SourceBufferKind::ReservationCeiling,
            unit: SourceBufferUnit::Bytes,
            requested: Some(1),
            bound: Some(limit),
            live_bytes: limit,
            bank_ceiling_bytes: limit,
        }
    );
    assert!(first.complete());
    assert_eq!(
        owner.poison_buffer_limit_at(
            SourceBufferSite::DnsCardinality,
            65,
            64,
            SourceBufferUnit::Entries
        ),
        SourceServiceError::BufferLimit
    );
    drop(held);
    assert_eq!(owner.snapshot().live_buffer_bytes, baseline);
    assert_eq!(origin(&recorder), first); // Creation-time live, not post-cleanup live.
}

#[test]
fn reservation_overflow_unrelated_first_and_unattributed_poison_stay_distinct() {
    let (owner, recorder) = armed();
    let baseline = owner.snapshot().live_buffer_bytes;
    assert_eq!(
        owner
            .reserve_buffer_at(usize::MAX, SourceBufferSite::RegistryArc)
            .err(),
        Some(SourceServiceError::BufferLimit)
    );
    let first = origin(&recorder);
    assert_eq!(first.kind, SourceBufferKind::ReservationOverflow);
    assert_eq!(first.requested, Some(usize::MAX));
    assert_eq!(first.live_bytes, baseline);
    assert!(first.complete());
    assert_eq!(owner.snapshot().live_buffer_bytes, baseline);
    let (owner, recorder) = armed();
    assert_eq!(
        owner.poison(SourceServiceError::Cancelled),
        SourceServiceError::Cancelled
    );
    assert_eq!(
        owner
            .reserve_buffer_at(usize::MAX, SourceBufferSite::SelectedPayload)
            .err(),
        Some(SourceServiceError::Cancelled)
    );
    assert_eq!(
        owner.poison_buffer_limit_at(
            SourceBufferSite::ArchiveUrlLength,
            8193,
            8192,
            SourceBufferUnit::Bytes
        ),
        SourceServiceError::Cancelled
    );
    assert!(recorder.snapshot().buffer.origin.is_none());
    assert!(!recorder.snapshot().buffer.first_error_is_buffer_limit);
    let (owner, recorder) = armed();
    owner.poison(SourceServiceError::BufferLimit);
    let first = origin(&recorder);
    assert_eq!(first.site, SourceBufferSite::Unattributed);
    assert_eq!(first.kind, SourceBufferKind::Unknown);
    assert_eq!(
        (first.requested, first.bound, first.unit),
        (None, None, SourceBufferUnit::Unknown)
    );
    assert!(!first.complete());
    owner.poison_buffer_limit_at(
        SourceBufferSite::ArchiveUrlLength,
        8193,
        8192,
        SourceBufferUnit::Bytes,
    );
    assert_eq!(origin(&recorder), first);
    let (owner, recorder) = armed();
    owner
        .reserve_buffer(owner.limits().live_buffer_bytes)
        .err()
        .unwrap();
    assert_eq!(origin(&recorder).kind, SourceBufferKind::ReservationCeiling);
    assert!(!origin(&recorder).complete()); // Unattributed reservation also proves no named site.
}

#[test]
fn natural_archive_length_and_dns_cardinality_producers_keep_exact_boundaries() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for length in [8192, 8193] {
        let (owner, recorder) = armed();
        let address = format!("file:/{}", "a".repeat(length - 6));
        assert_eq!(address.len(), length);
        assert!(
            crate::runtime::repository_archive_http::capture_selected_extension_artifact(
                &address,
                [0; 32],
                &runtime,
                &|| true,
                Some(&owner)
            )
            .is_err()
        );
        if length == 8192 {
            assert!(owner.check().is_ok()); // Ordinary invalid file path, not a Scope bound.
            assert!(recorder.snapshot().buffer.origin.is_none());
        } else {
            let first = origin(&recorder);
            assert_eq!(
                (first.site, first.kind, first.unit),
                (
                    SourceBufferSite::ArchiveUrlLength,
                    SourceBufferKind::ExplicitBound,
                    SourceBufferUnit::Bytes
                )
            );
            assert_eq!((first.requested, first.bound), (Some(8193), Some(8192)));
            assert!(first.complete());
            assert_eq!(owner.snapshot().source_attempts, 0);
        }
    }
    let address = "127.0.0.1:1".parse::<std::net::SocketAddr>().unwrap();
    for count in [64, 65] {
        let (owner, recorder) = armed();
        let result =
            super::super::http::bounded_addresses(&owner, std::iter::repeat_n(address, count));
        if count == 64 {
            assert_eq!(result.unwrap().len(), 64);
            assert!(recorder.snapshot().buffer.origin.is_none());
        } else {
            assert!(result.is_err());
            let first = origin(&recorder);
            assert_eq!(first.site, SourceBufferSite::DnsCardinality);
            assert_eq!(
                (first.kind, first.unit, first.requested, first.bound),
                (
                    SourceBufferKind::ExplicitBound,
                    SourceBufferUnit::Entries,
                    Some(65),
                    Some(64)
                )
            );
            assert!(first.complete());
        }
    }
}

#[test]
fn normal_file_download_records_payload_reservation_with_original_bank_facts() {
    let (owner, recorder) = armed();
    let size = 200 * 1024;
    let baseline = owner.snapshot().live_buffer_bytes;
    let limit = owner.limits().live_buffer_bytes;
    let held = owner
        .reserve_buffer(limit - baseline - (size + 64 - 1))
        .unwrap();
    let live = owner.snapshot().live_buffer_bytes;
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let chunk = [0x5a; 4096];
    let mut hash = Sha256::new();
    for _ in 0..50 {
        file.write_all(&chunk).unwrap();
        hash.update(chunk);
    }
    let digest: [u8; 32] = hash.finalize().into();
    let address = url::Url::from_file_path(file.path()).unwrap().to_string();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let error = crate::runtime::repository_archive_http::capture_selected_extension_artifact(
        &address,
        digest,
        &runtime,
        &|| true,
        Some(&owner),
    )
    .unwrap_err();
    assert_eq!(error, SourceServiceError::BufferLimit.to_string());
    assert_eq!(
        origin(&recorder),
        SourceBufferOrigin {
            site: SourceBufferSite::SelectedPayload,
            kind: SourceBufferKind::ReservationCeiling,
            unit: SourceBufferUnit::Bytes,
            requested: Some(size + 64),
            bound: Some(limit),
            live_bytes: live,
            bank_ceiling_bytes: limit,
        }
    );
    assert!(origin(&recorder).complete());
    assert_eq!(owner.snapshot().source_attempts, 1);
    assert_eq!(owner.snapshot().transferred_bytes, size as u64);
    assert_eq!(owner.snapshot().live_buffer_bytes, live); // All capture phase credits retired.
    drop(held);
    assert_eq!(owner.snapshot().live_buffer_bytes, baseline);
}

#[test]
fn recorder_charge_failed_arm_competing_drop_and_last_drop_cover_buffer_state() {
    let owner = scope(SourceServiceLimits {
        live_buffer_bytes: 1,
        ..SourceServiceLimits::default()
    });
    assert_eq!(
        owner.arm_registry_association_for_test().err(),
        Some(SourceServiceError::BufferLimit)
    );
    let snapshot = owner.registry_observation_for_test().2;
    assert!(snapshot.first_error_is_buffer_limit);
    assert!(!snapshot.armed && snapshot.origin.is_none());
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let owner = scope(SourceServiceLimits::default());
    assert_eq!(
        owner
            .arm_registry_association_with_hook(|| owner.cancel())
            .err(),
        Some(SourceServiceError::Cancelled)
    );
    assert!(!owner.registry_observation_for_test().2.armed);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let (owner, recorder) = armed();
    let charged = recorder.snapshot().charged_bytes;
    let competitor = RegistryRequestAssociation::new(&owner).unwrap();
    assert_eq!(owner.snapshot().live_buffer_bytes, 2 * charged);
    owner.poison_buffer_limit_at(
        SourceBufferSite::RedirectLength,
        8193,
        8192,
        SourceBufferUnit::Bytes,
    );
    let first = origin(&recorder);
    drop(competitor); // Its different Weak identity cannot disarm the installed winner.
    assert_eq!(origin(&recorder), first);
    assert_eq!(owner.snapshot().live_buffer_bytes, charged);
    let clone = recorder.clone();
    drop(recorder);
    assert!(owner.registry_observation_for_test().2.armed);
    drop(clone);
    let snapshot = owner.registry_observation_for_test().2;
    assert!(!snapshot.armed && snapshot.origin.is_none());
    assert!(snapshot.first_error_is_buffer_limit);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[test]
fn concurrent_reservation_and_explicit_bound_select_one_indivisible_origin() {
    let (owner, recorder) = armed();
    let live = owner.snapshot().live_buffer_bytes;
    let limit = owner.limits().live_buffer_bytes;
    let gate = Arc::new(Barrier::new(3));
    let first_owner = owner.clone();
    let first_gate = gate.clone();
    let reservation = std::thread::spawn(move || {
        first_gate.wait();
        first_owner
            .reserve_buffer_at(limit - live + 1, SourceBufferSite::SelectedPayload)
            .err()
            .unwrap()
    });
    let second_owner = owner.clone();
    let second_gate = gate.clone();
    let explicit = std::thread::spawn(move || {
        second_gate.wait();
        second_owner.poison_buffer_limit_at(
            SourceBufferSite::ArchiveUrlLength,
            8193,
            8192,
            SourceBufferUnit::Bytes,
        )
    });
    gate.wait();
    let results = (reservation.join(), explicit.join()); // Join both before assertions.
    assert_eq!(results.0.unwrap(), SourceServiceError::BufferLimit);
    assert_eq!(results.1.unwrap(), SourceServiceError::BufferLimit);
    let first = origin(&recorder);
    assert!(first.complete());
    assert_eq!(first.live_bytes, live);
    match first.site {
        SourceBufferSite::SelectedPayload => {
            assert_eq!(first.kind, SourceBufferKind::ReservationCeiling);
            assert_eq!(
                (first.requested, first.bound),
                (Some(limit - live + 1), Some(limit))
            );
        }
        SourceBufferSite::ArchiveUrlLength => {
            assert_eq!(first.kind, SourceBufferKind::ExplicitBound);
            assert_eq!((first.requested, first.bound), (Some(8193), Some(8192)));
        }
        _ => panic!("unowned first buffer origin"),
    }
}
