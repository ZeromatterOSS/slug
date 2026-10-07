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

    let scope_weak = Arc::downgrade(&scope);
    let bytes_weak = Arc::downgrade(&bytes);
    drop(bytes);
    drop(scope);
    assert!(bytes_weak.upgrade().is_none());
    assert!(scope_weak.upgrade().is_none());
}
