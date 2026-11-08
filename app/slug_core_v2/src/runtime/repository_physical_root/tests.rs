use std::time::Duration;
use std::time::Instant;

use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

pub(super) fn scope() -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/physical-root-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}

pub(super) fn resources() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(8 * 1024 * 1024, 16, 4 * ARCHIVE_BYTES).unwrap(),
    )
    .unwrap()
}

#[test]
fn resource_policy_and_bank_control_require_positive_exact_credits() {
    for (bytes, fds, backing) in [
        (0, 1, 1),
        (1, 0, 1),
        (1, 4097, 1),
        (1, 1, 0),
        (usize::MAX, 1, 1),
    ] {
        assert!(matches!(
            RepositoryRootResourceLimits::new(bytes, fds, backing),
            Err(PhysicalRootError::Resource)
        ));
    }
    let control = arc_bytes::<RepositoryRootResources>().unwrap() + 64;
    assert!(matches!(
        RepositoryRootResources::new(RepositoryRootResourceLimits::new(control - 1, 1, 1).unwrap()),
        Err(PhysicalRootError::Resource)
    ));
    let bank =
        RepositoryRootResources::new(RepositoryRootResourceLimits::new(control, 1, 1).unwrap())
            .unwrap();
    assert_eq!(bank.snapshot().retained_bytes, control);
    assert!(matches!(
        bank.reserve(1, 0, 0),
        Err(PhysicalRootError::Resource)
    ));
    assert_eq!(bank.snapshot().retained_bytes, control);
    assert_eq!(bank.snapshot().native_fds, 0);
    assert_eq!(bank.snapshot().backing_bytes, 0);
}

#[test]
fn resource_reservation_checks_all_domains_atomically_and_releases_on_last_owner() {
    let control = arc_bytes::<RepositoryRootResources>().unwrap() + 64;
    let bank = RepositoryRootResources::new(
        RepositoryRootResourceLimits::new(control + 8, 2, 16).unwrap(),
    )
    .unwrap();
    let first = bank.reserve(4, 1, 8).unwrap();
    assert_eq!(bank.snapshot().retained_bytes, control + 4);
    assert_eq!(bank.snapshot().native_fds, 1);
    assert_eq!(bank.snapshot().backing_bytes, 8);
    // Failure in each domain must roll back the earlier byte reservation and
    // leave the other counters unchanged; exact admission remains available.
    for (bytes, fds, backing) in [(5, 0, 0), (4, 2, 0), (4, 1, 9), (0, usize::MAX, u64::MAX)] {
        assert!(matches!(
            bank.reserve(bytes, fds, backing),
            Err(PhysicalRootError::Resource)
        ));
        let state = bank.snapshot();
        assert_eq!(
            (state.retained_bytes, state.native_fds, state.backing_bytes),
            (control + 4, 1, 8)
        );
    }
    let exact = bank.reserve(4, 1, 8).unwrap();
    let state = bank.snapshot();
    assert_eq!(
        (state.retained_bytes, state.native_fds, state.backing_bytes),
        (control + 8, 2, 16)
    );
    assert_eq!(
        (
            state.peak_retained_bytes,
            state.peak_native_fds,
            state.peak_backing_bytes
        ),
        (control + 8, 2, 16)
    );
    drop(first);
    assert_eq!(
        (
            bank.snapshot().retained_bytes,
            bank.snapshot().native_fds,
            bank.snapshot().backing_bytes
        ),
        (control + 4, 1, 8)
    );
    let weak = Arc::downgrade(&bank);
    drop(bank);
    assert!(weak.upgrade().is_some());
    drop(exact);
    assert!(weak.upgrade().is_none());
}

#[test]
fn ordinary_host_directories_and_regular_files_cannot_become_physical_roots() {
    let fixture = tempfile::tempdir().unwrap();
    let directory = File::open(fixture.path()).unwrap();
    let path = fixture.path().join("body");
    std::fs::write(&path, b"host").unwrap();
    for mode in [
        RepositoryWorkspaceMode::Archive,
        RepositoryWorkspaceMode::Process,
    ] {
        assert!(matches!(
            backing_identity(&directory, mode),
            Err(PhysicalRootError::RootIdentity) | Err(PhysicalRootError::Backing)
        ));
        assert!(matches!(
            backing_identity(&File::open(&path).unwrap(), mode),
            Err(PhysicalRootError::RootIdentity)
        ));
    }
}

#[test]
fn scope_refusal_precedes_inactive_authority_and_allocations() {
    let owner = scope();
    assert!(matches!(
        check(&owner, &|| false),
        Err(PhysicalRootError::Inactive)
    ));
    assert_eq!(owner.snapshot().source_attempts, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    owner.cancel();
    assert!(matches!(
        check(&owner, &|| false),
        Err(PhysicalRootError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    assert_eq!(owner.snapshot().source_attempts, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}
