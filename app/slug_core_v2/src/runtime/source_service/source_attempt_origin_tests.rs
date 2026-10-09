use std::cell::Cell;

use slug_workspace_v2::NormalizedAbsolutePath;

use super::super::regular_file::RegularFileError;
use super::super::regular_file::RegularFileStage;
use super::super::regular_file::read_host_file;
use super::super::regular_file::read_host_file_with_charge_origin;
use super::*;

fn owner() -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new(
        NormalizedAbsolutePath::new("/source-attempt-origin-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits {
            source_attempts: 1,
            ..SourceServiceLimits::default()
        },
    )
    .unwrap()
}

#[test]
fn source_attempt_origin_is_first_created_only() {
    let scope = owner();
    let limit = SourceServiceError::Limit(SourceCounter::SourceAttempts);
    assert_eq!(scope.charge_source_attempt_with_origin(), (Ok(()), false));
    assert_eq!(scope.snapshot().source_attempts, 1);
    assert_eq!(
        scope.charge_source_attempt_with_origin(),
        (Err(limit), true)
    );
    assert_eq!(
        scope.charge_source_attempt_with_origin(),
        (Err(limit), false)
    );
    assert_eq!(scope.snapshot().source_attempts, 1);
    assert_eq!(scope.check(), Err(limit));
    assert_eq!(scope.poison(SourceServiceError::Allocation), limit);

    for error in [
        SourceServiceError::Cancelled,
        SourceServiceError::AcquisitionClosed,
        SourceServiceError::Allocation,
        SourceServiceError::NetworkDisabled,
        SourceServiceError::WorkDeadline,
    ] {
        let mut scope = owner();
        match error {
            SourceServiceError::Cancelled => scope.cancel(),
            SourceServiceError::AcquisitionClosed => scope.close_acquisition().unwrap(),
            SourceServiceError::WorkDeadline => {
                Arc::get_mut(&mut scope).unwrap().work_deadline = Instant::now();
            }
            _ => {
                scope.poison(error);
            }
        }
        assert_eq!(
            scope.charge_source_attempt_with_origin(),
            (Err(error), false)
        );
        assert_eq!(
            scope.charge_source_attempt_with_origin(),
            (Err(error), false)
        );
        assert_eq!(scope.snapshot().source_attempts, 0);
    }

    // Private corruption fixture discriminates overflow without a public mutator.
    let scope = owner();
    scope.state().counters.source_attempts = u64::MAX;
    let overflow = SourceServiceError::Overflow(SourceCounter::SourceAttempts);
    assert_eq!(
        scope.charge_source_attempt_with_origin(),
        (Err(overflow), false)
    );
    assert_eq!(scope.snapshot().source_attempts, u64::MAX);

    let scope = owner();
    scope.charge(SourceCounter::SourceAttempts, 1).unwrap();
    let start = std::sync::Barrier::new(2);
    let results = std::thread::scope(|threads| {
        let threads = (0..2)
            .map(|_| {
                let scope = &scope;
                let start = &start;
                threads.spawn(move || {
                    start.wait();
                    scope.charge_source_attempt_with_origin()
                })
            })
            .collect::<Vec<_>>();
        threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(results.iter().all(|(result, _)| *result == Err(limit)));
    assert_eq!(results.iter().filter(|(_, created)| *created).count(), 1);
    assert_eq!(scope.snapshot().source_attempts, 1);
    use super::super::SourceChargeDomain;
    use super::super::SourceChargeIdentity;
    use super::super::SourceChargePhase;
    use super::super::source_charge_census::SourceChargeOutcome;
    let scope = census_owner();
    let (result, first) = scope.charge_source_with_record_for_test(1, SourceChargeDomain::Host);
    assert_eq!(result, Ok(()));
    let first = first.unwrap();
    assert_eq!(
        (
            first.event,
            first.before,
            first.after,
            first.next,
            first.first_created
        ),
        (1, 0, 1, Some(1), false)
    );
    let start = std::sync::Barrier::new(2);
    let records = std::thread::scope(|threads| {
        let a = threads.spawn(|| {
            start.wait();
            scope.charge_source_with_record_for_test(1, SourceChargeDomain::Host)
        });
        let b = threads.spawn(|| {
            start.wait();
            scope.charge_source_with_record_for_test(1, SourceChargeDomain::Host)
        });
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert!(records.iter().all(|(result, _)| *result == Err(limit)));
    let a = records[0].1.unwrap();
    let b = records[1].1.unwrap();
    assert_ne!(a.event, b.event);
    assert_eq!(a.event + b.event, 5);
    assert_eq!(
        usize::from(a.first_created) + usize::from(b.first_created),
        1
    );
    for record in [a, b] {
        assert_eq!(
            record.outcome,
            if record.first_created {
                SourceChargeOutcome::FirstSourceLimit
            } else {
                SourceChargeOutcome::StickySourceLimit
            }
        );
        assert_eq!((record.before, record.after, record.next), (1, 1, Some(2)));
    }
    // Reports may arrive in reverse event order; accounting remains lock-local.
    for record in [b, a, first] {
        scope.report_source_charge_for_test(
            record,
            SourceChargePhase::Unassociated,
            SourceChargeIdentity::Anonymous,
        );
    }
    assert!(
        format!("{}", scope.state().census.as_ref().unwrap().summary(1, 1)).contains(&format!(
            "first_limit={} ",
            if a.first_created { a.event } else { b.event }
        ))
    );
    let (result, record) = owner().charge_source_with_record_for_test(1, SourceChargeDomain::Host);
    assert_eq!(result, Ok(()));
    assert!(record.is_none());
    let cancelled = census_owner();
    cancelled.cancel();
    let (result, record) =
        cancelled.charge_source_with_record_for_test(1, SourceChargeDomain::Host);
    assert_eq!(result, Err(SourceServiceError::Cancelled));
    let record = record.unwrap();
    assert_eq!(record.outcome, SourceChargeOutcome::OtherError);
    assert!(!record.first_created);
    let overflow = census_owner();
    overflow.state().counters.source_attempts = u64::MAX;
    let (result, record) = overflow.charge_source_with_record_for_test(1, SourceChargeDomain::Host);
    assert_eq!(
        result,
        Err(SourceServiceError::Overflow(SourceCounter::SourceAttempts))
    );
    let record = record.unwrap();
    assert_eq!(record.outcome, SourceChargeOutcome::Overflow);
    assert_eq!(record.next, None);
}

#[test]
fn host_source_attempt_origin_callback_is_borrowed_lock_free_and_pre_io() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("body");
    std::fs::write(&path, b"x").unwrap();
    let scope = owner();
    let calls = Cell::new(0);
    let bytes =
        read_host_file_with_charge_origin(&scope, &path, || calls.set(calls.get() + 1)).unwrap();
    assert_eq!(bytes.as_ref(), b"x");
    assert_eq!(calls.get(), 0);
    assert_eq!(scope.snapshot().decoded_bytes, 1);
    let plain_scope = owner();
    let plain = read_host_file(&plain_scope, &path).unwrap();
    assert_eq!(plain.as_ref(), bytes.as_ref());
    let plain_counters = plain_scope.snapshot();
    let counters = scope.snapshot();
    assert_eq!(
        (
            plain_counters.source_attempts,
            plain_counters.decoded_bytes,
            plain_counters.live_buffer_bytes,
            plain_counters.peak_buffer_bytes
        ),
        (
            counters.source_attempts,
            counters.decoded_bytes,
            counters.live_buffer_bytes,
            counters.peak_buffer_bytes
        ),
    );

    let missing = directory.path().join("never-opened");
    let borrowed = missing.as_path();
    let original = borrowed.as_os_str().as_encoded_bytes().as_ptr();
    let limit = SourceServiceError::Limit(SourceCounter::SourceAttempts);
    assert!(
        matches!(read_host_file_with_charge_origin(&scope, borrowed, || {
        // This would deadlock if the facade called us under the scope mutex.
        assert_eq!(scope.snapshot().source_attempts, 1);
        assert_eq!(scope.check(), Err(limit));
        assert_eq!(borrowed.as_os_str().as_encoded_bytes().as_ptr(), original);
        calls.set(calls.get() + 1);
    }), Err(RegularFileError::Scope(error)) if error == limit)
    );
    assert_eq!(calls.get(), 1);
    assert!(
        matches!(read_host_file_with_charge_origin(&scope, &directory.path().join("different"), || {
        calls.set(calls.get() + 1);
    }), Err(RegularFileError::Scope(error)) if error == limit)
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(scope.snapshot().decoded_bytes, 1);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);

    let missing_scope = owner();
    assert!(
        matches!(read_host_file_with_charge_origin(&missing_scope, borrowed, || {
        calls.set(calls.get() + 1);
    }), Err(RegularFileError::Io { stage: RegularFileStage::Pin, error, .. })
        if error.kind() == std::io::ErrorKind::NotFound)
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(missing_scope.snapshot().source_attempts, 1);
    assert_eq!(missing_scope.snapshot().decoded_bytes, 0);
    assert_eq!(missing_scope.check(), Ok(()));

    use slug_workspace_v2::PathObservationDemand;
    use slug_workspace_v2::PathObservationNamespace;
    use slug_workspace_v2::PathObservationOperation;

    use super::super::SourceChargeIdentity;
    use super::super::SourceChargePhase;
    use super::super::read_host_file_with_known_shas_allowance_and_census;
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(&path).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let census = census_owner();
    let order = Cell::new(0);
    let captured = read_host_file_with_known_shas_allowance_and_census(
        &census,
        &demand,
        || panic!("admitted read"),
        |record| {
            assert_eq!(census.snapshot().source_attempts, 1);
            assert_eq!(census.snapshot().decoded_bytes, 0);
            assert_eq!(record.event, 1);
            assert!(!record.first_created);
            order.set(1);
            census.report_source_charge_for_test(
                record,
                SourceChargePhase::NeedPaths,
                SourceChargeIdentity::Demand(&demand),
            );
        },
    )
    .unwrap();
    assert_eq!(captured.as_ref(), plain.as_ref());
    assert_eq!(order.get(), 1);
    let rejected = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(&missing).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let pointer = rejected
        .path()
        .as_path()
        .as_os_str()
        .as_encoded_bytes()
        .as_ptr();
    assert!(
        matches!(read_host_file_with_known_shas_allowance_and_census(&census,&rejected,|| { assert_eq!(order.get(),2); order.set(3); },|record| {
        assert_eq!(order.get(),1); assert!(record.first_created); assert_eq!(census.check(),Err(limit));
        assert_eq!(rejected.path().as_path().as_os_str().as_encoded_bytes().as_ptr(),pointer);
        assert_eq!(census.snapshot().decoded_bytes,1); order.set(2);
        census.report_source_charge_for_test(record,SourceChargePhase::NeedPaths,SourceChargeIdentity::Demand(&rejected));
    }),Err(RegularFileError::Scope(error)) if error==limit)
    );
    assert_eq!(order.get(), 3);
    let off = owner();
    let disabled_calls = Cell::new(0);
    let off_bytes = read_host_file_with_known_shas_allowance_and_census(
        &off,
        &demand,
        || panic!("admitted"),
        |_| disabled_calls.set(1),
    )
    .unwrap();
    assert_eq!(disabled_calls.get(), 0);
    assert_eq!(off_bytes.as_ref(), captured.as_ref());
    let disabled_first = Cell::new(false);
    assert!(
        matches!(read_host_file_with_known_shas_allowance_and_census(&off, &rejected, || disabled_first.set(true), |_| disabled_calls.set(1)), Err(RegularFileError::Scope(error)) if error == limit)
    );
    assert!(disabled_first.get());
    assert_eq!(disabled_calls.get(), 0);
    let missing_census = census_owner();
    assert!(
        matches!(read_host_file_with_known_shas_allowance_and_census(&missing_census,&rejected,|| panic!("no quota failure"),|record| {
        assert_eq!(missing_census.snapshot().decoded_bytes,0); assert!(!record.first_created);
        missing_census.report_source_charge_for_test(record,SourceChargePhase::CertificatePrecheck,SourceChargeIdentity::Demand(&rejected));
    }),Err(RegularFileError::Io { stage:RegularFileStage::Pin,error,.. }) if error.kind()==std::io::ErrorKind::NotFound)
    );
    assert_eq!(missing_census.snapshot().source_attempts, 1);
    let census_weak = Arc::downgrade(&census);
    let captured_weak = Arc::downgrade(&captured);
    drop(captured);
    drop(census);
    assert!(captured_weak.upgrade().is_none());
    assert!(census_weak.upgrade().is_none());
    let scope_weak = Arc::downgrade(&scope);
    let bytes_weak = Arc::downgrade(&bytes);
    drop(bytes);
    drop(scope);
    assert!(bytes_weak.upgrade().is_none());
    assert!(scope_weak.upgrade().is_none());
}

fn census_owner() -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new_for_test_with_source_charge_census(
        NormalizedAbsolutePath::new("/source-attempt-origin-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits {
            source_attempts: 1,
            ..SourceServiceLimits::default()
        },
        None,
    )
    .unwrap()
}
