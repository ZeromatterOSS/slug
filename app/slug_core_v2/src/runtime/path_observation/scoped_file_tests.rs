//! Focused Linux controls for the original epoch and scoped refusal boundary.
use std::time::Duration;
use std::time::Instant;

use super::*;
use crate::runtime::dice::source_service::HOST_FILE_OBJECT_BYTES;
use crate::runtime::dice::source_service::SourceCounter;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn scope(root: &NormalizedAbsolutePath) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new_for_test(
        root.clone(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap()
}

fn bytes_demand(path: &std::path::Path) -> PathObservationDemand {
    PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(path.to_path_buf()).unwrap(),
        PathObservationOperation::FileBytes,
    )
}

#[test]
fn scoped_file_epoch_preserves_bytes_missing_directory_and_original_arcs() {
    let directory = tempfile::tempdir().unwrap();
    let root = NormalizedAbsolutePath::new(directory.path().to_path_buf()).unwrap();
    let file = directory.path().join("body");
    let absent = directory.path().join("absent");
    let bytes = b"a\0b\xff";
    std::fs::write(&file, bytes).unwrap();
    let demands = [
        bytes_demand(&file),
        bytes_demand(&absent),
        bytes_demand(directory.path()),
    ];
    let owner = scope(&root);
    let observed = observe_native_scoped(&directory, [], demands.clone(), Some(&owner)).unwrap();
    let ordinary = observe_native(&directory, [], demands.clone()).unwrap();
    assert_eq!(observed, ordinary);
    match observed.get(&demands[0]).unwrap().as_ref() {
        PathObservationResult::FileBytes(PathOperationResult::Present(body)) => {
            assert_eq!(body.as_ref(), bytes);
            let retained = body.clone();
            assert!(Arc::ptr_eq(body, &retained));
        }
        value => panic!("unexpected file observation: {value:?}"),
    }
    assert_eq!(
        observed.get(&demands[1]).unwrap().as_ref(),
        &PathObservationResult::FileBytes(PathOperationResult::Missing)
    );
    assert_eq!(
        observed.get(&demands[2]).unwrap().as_ref(),
        &PathObservationResult::FileBytes(PathOperationResult::Error(
            PathObservationError::WrongKind {
                expected: PathNodeKind::RegularFile,
                actual: PathNodeKind::Directory,
            }
        ))
    );
    let counters = owner.snapshot();
    assert_eq!(counters.source_attempts, 3);
    assert_eq!(counters.decoded_bytes, bytes.len() as u64);
    assert_eq!(counters.registry_attempts, 0);
    assert_eq!(counters.registry_admitted_bytes, 0);
    assert_eq!(counters.transferred_bytes, 0);
    assert_eq!(counters.written_bytes, 0);
    assert_eq!(counters.live_buffer_bytes, 0);
}

#[test]
fn scoped_file_epoch_validates_whole_demand_set_and_aborts_operational_failures() {
    let directory = tempfile::tempdir().unwrap();
    let root = NormalizedAbsolutePath::new(directory.path().to_path_buf()).unwrap();
    let file = directory.path().join("body");
    std::fs::write(&file, b"value").unwrap();
    let valid = bytes_demand(&file);
    let unknown = PathObservationDemand::new(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(9)),
        root.clone(),
        PathObservationOperation::Lstat,
    );
    let owner = scope(&root);
    assert_eq!(
        observe_native_scoped(
            &directory,
            [],
            [valid.clone(), unknown.clone()],
            Some(&owner)
        ),
        Err(PathObservationKernelError::UnknownDemandInstance(unknown))
    );
    assert_eq!(owner.snapshot().source_attempts, 0);
    assert_eq!(
        observe_native_scoped(&directory, [], [valid.clone(), valid.clone()], Some(&owner)),
        Err(PathObservationKernelError::DuplicateDemand(valid.clone()))
    );
    assert_eq!(owner.snapshot().source_attempts, 0);
    owner.cancel();
    assert_eq!(
        observe_native_scoped(&directory, [], [valid.clone()], Some(&owner)),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Cancelled
        ))
    );
    assert_eq!(owner.snapshot().source_attempts, 0);
    let owner = scope(&root);
    std::fs::File::create(&file)
        .unwrap()
        .set_len((HOST_FILE_OBJECT_BYTES + 1) as u64)
        .unwrap();
    assert_eq!(
        observe_native_scoped(&directory, [], [valid], Some(&owner)),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes)
        ))
    );
    assert_eq!(owner.snapshot().source_attempts, 1);
    assert_eq!(owner.snapshot().decoded_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[test]
fn source_attempt_callback_preserves_whole_set_first_origin_and_sticky_refusal() {
    use std::cell::Cell;
    use std::os::unix::ffi::OsStrExt;

    let directory = tempfile::tempdir().unwrap();
    let root = NormalizedAbsolutePath::new(directory.path().to_path_buf()).unwrap();
    let absent = bytes_demand(&directory.path().join("absent"));
    let original_path = absent.path().as_path().as_os_str().as_bytes().as_ptr();
    let calls = Cell::new(0);
    let owner = scope(&root);
    owner.charge(SourceCounter::SourceAttempts, 64).unwrap();
    let unknown = PathObservationDemand::new(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(9)),
        root.clone(),
        PathObservationOperation::Lstat,
    );
    // Validation of the entire set precedes any charge or callback.
    assert_eq!(
        observe_native_scoped_with_charge_origin(
            &directory,
            [],
            [absent.clone(), unknown.clone()],
            Some(&owner),
            &mut |_| calls.set(calls.get() + 1),
        ),
        Err(PathObservationKernelError::UnknownDemandInstance(unknown))
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(owner.snapshot().source_attempts, 64);
    owner.check().unwrap();

    let refused = PathObservationKernelError::SourceService(SourceServiceError::Limit(
        SourceCounter::SourceAttempts,
    ));
    assert_eq!(
        observe_native_scoped_with_charge_origin(
            &directory,
            [],
            [absent.clone()],
            Some(&owner),
            &mut |demand| {
                calls.set(calls.get() + 1);
                assert_eq!(demand, &absent);
                assert_eq!(
                    demand.path().as_path().as_os_str().as_bytes().as_ptr(),
                    original_path
                );
                assert_eq!(owner.snapshot().source_attempts, 64);
                assert_eq!(owner.snapshot().decoded_bytes, 0);
                assert!(!absent.path().as_path().exists());
            },
        ),
        Err(refused)
    );
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        observe_native_scoped_with_charge_origin(
            &directory,
            [],
            [absent.clone()],
            Some(&owner),
            &mut |_| calls.set(calls.get() + 1),
        ),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Limit(SourceCounter::SourceAttempts)
        ))
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);

    let healthy = scope(&root);
    let observed = observe_native_scoped_with_charge_origin(
        &directory,
        [],
        [absent.clone()],
        Some(&healthy),
        &mut |_| calls.set(calls.get() + 1),
    )
    .unwrap();
    assert_eq!(
        observed.get(&absent).unwrap().as_ref(),
        &PathObservationResult::FileBytes(PathOperationResult::Missing)
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(healthy.snapshot().source_attempts, 1);
    healthy.cancel();
    assert_eq!(
        observe_native_scoped_with_charge_origin(
            &directory,
            [],
            [absent],
            Some(&healthy),
            &mut |_| calls.set(calls.get() + 1),
        ),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Cancelled
        ))
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn known_shas_host_object_allowance_is_exact_and_scoped() {
    use crate::runtime::dice::source_service::KNOWN_SHAS_OBJECT_BYTES;

    let directory = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(directory.path().to_path_buf()).unwrap();
    let root = NormalizedAbsolutePath::new(directory.path().join("rules")).unwrap();
    let file = root.as_path().join("rust/private/known_shas.bzl");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let input = std::fs::File::create(&file).unwrap();
    input.set_len(KNOWN_SHAS_OBJECT_BYTES as u64).unwrap();
    drop(input);
    let selected = |limits| {
        let deadline = Instant::now() + Duration::from_secs(2);
        SourceServiceScope::new_for_test_with_known_shas_allowance(
            workspace.clone(),
            deadline,
            deadline + Duration::from_secs(1),
            limits,
            root.clone(),
        )
        .unwrap()
    };
    let demand = bytes_demand(&file);
    let default = scope(&workspace);
    assert!(default.known_shas_allowance_for_test().is_none());
    assert!(default.known_shas_object_bytes_for_test(&demand).is_none());
    assert_eq!(
        observe_native_scoped(&directory, [], [demand.clone()], Some(&default)),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes,)
        ))
    );
    assert_eq!(default.snapshot().decoded_bytes, 0);

    let owner = selected(SourceServiceLimits::default());
    let (borrowed_root, maximum) = owner.known_shas_allowance_for_test().unwrap();
    assert_eq!(borrowed_root, &root);
    assert!(std::ptr::eq(borrowed_root.as_path(), root.as_path()));
    assert_eq!(maximum, KNOWN_SHAS_OBJECT_BYTES);
    assert_eq!(owner.limits().registry_object_bytes, 1 << 20);
    assert_eq!(
        owner.known_shas_object_bytes_for_test(&demand),
        Some(maximum)
    );
    for operation in [
        PathObservationOperation::Lstat,
        PathObservationOperation::FileDigest,
        PathObservationOperation::DirectoryEntries,
        PathObservationOperation::ReadLink,
    ] {
        let invalid = PathObservationDemand::new(
            PathObservationNamespace::Host,
            demand.path().clone(),
            operation,
        );
        assert!(owner.known_shas_object_bytes_for_test(&invalid).is_none());
    }
    for relative in [
        "rust/private/known_shas.bzl/child",
        "rust/private/known_shas.bzl.extra",
        "rust/private/KNOWN_SHAS.bzl",
        "rust/private/nightly_versions.bzl",
    ] {
        assert!(
            owner
                .known_shas_object_bytes_for_test(&bytes_demand(&root.as_path().join(relative)))
                .is_none()
        );
    }
    let wrong_root = bytes_demand(
        &directory
            .path()
            .join("rules-other/rust/private/known_shas.bzl"),
    );
    assert!(
        owner
            .known_shas_object_bytes_for_test(&wrong_root)
            .is_none()
    );
    let materialized = PathObservationDemand::new(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
        demand.path().clone(),
        PathObservationOperation::FileBytes,
    );
    assert!(
        owner
            .known_shas_object_bytes_for_test(&materialized)
            .is_none()
    );
    // This instance is known. Refusal must come from the default object cap,
    // not from the whole-set unknown-instance check.
    let isolated = selected(SourceServiceLimits::default());
    assert_eq!(
        observe_native_scoped(
            &directory,
            [(PathObservationInstanceId::new(1), root.clone())],
            [materialized],
            Some(&isolated),
        ),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes,)
        ))
    );
    assert_eq!(isolated.snapshot().decoded_bytes, 0);

    let epoch = observe_native_scoped(&directory, [], [demand.clone()], Some(&owner)).unwrap();
    let PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) =
        epoch.get(&demand).unwrap().as_ref()
    else {
        panic!("selected Host source did not produce bytes");
    };
    assert_eq!(bytes.len(), maximum);
    assert!(bytes.iter().all(|byte| *byte == 0));
    let held = bytes.clone();
    assert!(Arc::ptr_eq(bytes, &held));
    let weak = Arc::downgrade(bytes);
    assert_eq!(owner.snapshot().decoded_bytes, maximum as u64);
    assert_eq!(owner.snapshot().source_attempts, 1);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    drop(epoch);
    assert!(weak.upgrade().is_some());
    let scope_weak = Arc::downgrade(&owner);
    drop(owner);
    assert!(scope_weak.upgrade().is_none());
    assert_eq!(held.len(), maximum);
    drop(held);
    assert!(weak.upgrade().is_none());

    // The selected maximum does not supersede cumulative byte history.
    let limited = selected(SourceServiceLimits {
        decoded_bytes: 1,
        ..SourceServiceLimits::default()
    });
    assert_eq!(
        observe_native_scoped(&directory, [], [demand.clone()], Some(&limited)),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes,)
        ))
    );
    assert_eq!(limited.snapshot().decoded_bytes, 0);
    assert_eq!(limited.snapshot().live_buffer_bytes, 0);

    // One oversized selected member aborts the original whole-set operation;
    // it cannot return an epoch containing the other successfully read member.
    let other = root.as_path().join("a");
    std::fs::write(&other, b"x").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_len(KNOWN_SHAS_OBJECT_BYTES as u64 + 1)
        .unwrap();
    let oversized = selected(SourceServiceLimits::default());
    assert_eq!(
        observe_native_scoped(
            &directory,
            [],
            [bytes_demand(&other), demand],
            Some(&oversized)
        ),
        Err(PathObservationKernelError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes,)
        ))
    );
    assert_eq!(oversized.snapshot().source_attempts, 2);
    assert_eq!(oversized.snapshot().decoded_bytes, 1);
    assert_eq!(oversized.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        oversized.check(),
        Err(SourceServiceError::Limit(SourceCounter::DecodedBytes))
    );
}
