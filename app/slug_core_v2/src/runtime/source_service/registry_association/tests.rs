use std::ffi::CString;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::ffi::OsStringExt;
use std::sync::Barrier;
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_bzlmod_v2::RegistryRefusalCause;
use slug_bzlmod_v2::RegistryRefusalDetail;
use slug_bzlmod_v2::RegistryRefusalScopeCounter;
use slug_bzlmod_v2::RegistryRefusalScopeFailure;
use slug_bzlmod_v2::RegistryRefusalSite;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::dice::source_service::ScopedRegistryIo;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn scope(limit: u64) -> Arc<SourceServiceScope> {
    let limits = SourceServiceLimits {
        registry_attempts: limit,
        ..SourceServiceLimits::default()
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/registry-association-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}
fn url(value: &str) -> RegistryFileUrl {
    RegistryFileUrl::new(value.to_owned())
}
fn charge(scope: &SourceServiceScope, value: &RegistryFileUrl) -> Result<(), SourceServiceError> {
    scope.charge_registry_request_for_test(RegistryRequestRef::Url(value))
}

#[tokio::test]
async fn exact_registry_routes_repeats_supplied_non_utf8_paths_and_unarmed_controls() {
    let directory = tempfile::tempdir().unwrap();
    let a = directory
        .path()
        .join(std::ffi::OsString::from_vec(b"a-\xff".to_vec()));
    let b = directory.path().join("b");
    std::fs::write(&a, b"x").unwrap();
    std::fs::write(&b, b"x").unwrap();
    let owner = scope(128);
    let adapter = ScopedRegistryIo::new(owner.clone());
    let display = url("file:///display-only");
    assert_eq!(
        adapter.read_local_exact(&display, &a).await.unwrap(),
        RegistryIoOutcome::Found(Arc::from(&b"x"[..]))
    );
    assert_eq!(owner.snapshot().registry_attempts, 1);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert!(matches!(
        owner.arm_registry_association_for_test(),
        Err(SourceServiceError::ForeignScope)
    ));
    let owner = scope(128);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    let adapter = ScopedRegistryIo::new(owner.clone());
    for path in [&a, &a, &b] {
        adapter.read_local_exact(&display, path).await.unwrap();
    }
    adapter
        .read_local_exact(&url("file:///different-display"), &a)
        .await
        .unwrap();
    // Url route is a distinct invocation domain from supplied Local path.
    let file_url = RegistryFileUrl::new(url::Url::from_file_path(&b).unwrap().to_string());
    adapter.read_exact(&file_url).await.unwrap();
    let observed = ledger.snapshot();
    assert!(observed.complete && ledger.same_scope(&owner));
    assert!(!ledger.same_scope(&scope(1)));
    assert_eq!(
        (
            observed.attempted_entries,
            observed.admitted_entries,
            observed.scope_admitted_entries
        ),
        (5, 5, 5)
    );
    assert_eq!(
        (
            observed.admitted_distinct,
            observed.admitted_repeated,
            observed.retained_rows
        ),
        (4, 1, 4)
    );
    assert_eq!(owner.snapshot().registry_admitted_bytes, 5);
    assert_eq!(owner.snapshot().live_buffer_bytes, observed.charged_bytes);
    drop(ledger);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    // Instrumentation bounds preserve the adapter's own URL and path outcomes.
    let owner = scope(2);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    let adapter = ScopedRegistryIo::new(owner.clone());
    assert!(
        adapter
            .read_local_exact(&display, Path::new(&"p".repeat(8193)))
            .await
            .is_err()
    );
    assert!(owner.check().is_ok());
    assert_eq!(owner.snapshot().registry_attempts, 1);
    assert!(!ledger.snapshot().complete);
    let oversized_url = url(&format!("http://example.invalid/{}", "x".repeat(8193)));
    assert!(adapter.read_exact(&oversized_url).await.is_err());
    assert_eq!(owner.check(), Err(SourceServiceError::BufferLimit));
    assert_eq!(owner.snapshot().registry_attempts, 2);
    assert_eq!(owner.snapshot().registry_admitted_bytes, 0);
}

fn watch(path: &Path) -> std::fs::File {
    let descriptor = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    assert!(descriptor >= 0);
    let file = unsafe { std::fs::File::from_raw_fd(descriptor) };
    let name = CString::new(path.as_os_str().as_bytes()).unwrap();
    assert!(
        unsafe {
            libc::inotify_add_watch(
                file.as_raw_fd(),
                name.as_ptr(),
                libc::IN_OPEN | libc::IN_ACCESS,
            )
        } >= 0
    );
    file
}
fn drain_events(file: &mut std::fs::File) -> u32 {
    let mut events = 0;
    let mut buffer = [0u8; 4096];
    for _ in 0..32 {
        match file.read(&mut buffer) {
            Ok(0) => panic!("inotify unexpectedly closed"),
            Ok(count) => {
                let mut offset = 0;
                while offset < count {
                    assert!(count - offset >= std::mem::size_of::<libc::inotify_event>());
                    let event = unsafe {
                        std::ptr::read_unaligned(
                            buffer[offset..].as_ptr().cast::<libc::inotify_event>(),
                        )
                    };
                    assert_eq!(event.mask & libc::IN_Q_OVERFLOW, 0);
                    events |= event.mask;
                    offset += std::mem::size_of::<libc::inotify_event>() + event.len as usize;
                    assert!(offset <= count);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return events,
            Err(e) => panic!("inotify read failed: {e}"),
        }
    }
    panic!("bounded inotify drain exhausted")
}

#[tokio::test]
async fn registry_admission_128_then_129_repeat_or_new_has_no_source_open_or_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source");
    std::fs::write(&path, b"x").unwrap();
    let mut sentinel = watch(&path);
    for repeated in [true, false] {
        let owner = scope(128);
        let ledger = owner.arm_registry_association_for_test().unwrap();
        let adapter = ScopedRegistryIo::new(owner.clone());
        let original = url("file:///display-one");
        adapter.read_local_exact(&original, &path).await.unwrap();
        let positive = drain_events(&mut sentinel);
        assert_ne!(positive & libc::IN_OPEN, 0);
        assert_ne!(positive & libc::IN_ACCESS, 0);
        for _ in 1..128 {
            adapter.read_local_exact(&original, &path).await.unwrap();
        }
        assert_ne!(drain_events(&mut sentinel) & libc::IN_OPEN, 0);
        let before = owner.snapshot();
        let refused = if repeated {
            original
        } else {
            url("file:///display-new")
        };
        let error = adapter.read_local_exact(&refused, &path).await.unwrap_err();
        assert_eq!(
            error.cause,
            Some(RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::Limit(
                    RegistryRefusalScopeCounter::RegistryAttempts
                ))
            })
        );
        assert_eq!(drain_events(&mut sentinel), 0);
        let after = owner.snapshot();
        assert_eq!(after.registry_attempts, 128);
        assert_eq!(
            after.registry_admitted_bytes,
            before.registry_admitted_bytes
        );
        assert_eq!(after.live_buffer_bytes, before.live_buffer_bytes);
        assert_eq!(after.peak_buffer_bytes, before.peak_buffer_bytes);
        let observed = ledger.snapshot();
        assert!(observed.complete);
        assert_eq!(
            (
                observed.admitted_distinct,
                observed.admitted_repeated,
                observed.retained_rows
            ),
            (1, 127, 1)
        );
        assert_eq!(
            (
                observed.attempted_entries,
                observed.refused_entries,
                observed.first_limit_call_ordinal
            ),
            (129, 1, Some(129))
        );
        let expected = if repeated {
            RegistryLimitRelation::ExistingRow {
                row_ordinal: 1,
                prior_admitted_count: 128,
            }
        } else {
            RegistryLimitRelation::NewDistinctUnretained { unique_ordinal: 2 }
        };
        assert_eq!(observed.first_limit_relation, expected);
        assert!(adapter.read_local_exact(&refused, &path).await.is_err());
        owner.cancel();
        let final_state = ledger.snapshot();
        assert_eq!(final_state.first_limit_relation, expected);
        assert_eq!(final_state.first_limit_call_ordinal, Some(129));
        assert_eq!(final_state.sticky_refused_entries, 1);
        assert_eq!(drain_events(&mut sentinel), 0);
        drop(ledger);
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
}

#[tokio::test]
async fn actual_root_256_distinct_then_257_refuses_before_io_and_preserves_origin() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("source");
    std::fs::write(&path, b"x").unwrap();
    let mut sentinel = watch(&path);
    let owner = scope(SourceServiceLimits::actual_root().registry_attempts);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    let adapter = ScopedRegistryIo::new(owner.clone());
    for ordinal in 1..=256 {
        adapter
            .read_local_exact(&url(&format!("file:///identity-{ordinal}")), &path)
            .await
            .unwrap();
    }
    let positive = drain_events(&mut sentinel);
    assert_ne!(positive & libc::IN_OPEN, 0);
    assert_ne!(positive & libc::IN_ACCESS, 0);
    let before = owner.snapshot();
    let admitted = ledger.snapshot();
    assert!(admitted.complete);
    assert_eq!(
        (
            admitted.admitted_entries,
            admitted.admitted_distinct,
            admitted.admitted_repeated,
            admitted.retained_rows
        ),
        (256, 256, 0, 256)
    );
    for ordinal in [257, 1] {
        let error = adapter
            .read_local_exact(&url(&format!("file:///identity-{ordinal}")), &path)
            .await
            .unwrap_err();
        assert_eq!(
            error.cause,
            Some(RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::Limit(
                    RegistryRefusalScopeCounter::RegistryAttempts
                )),
            })
        );
        assert_eq!(drain_events(&mut sentinel), 0);
        let after = owner.snapshot();
        assert_eq!(after.registry_attempts, 256);
        assert_eq!(
            after.registry_admitted_bytes,
            before.registry_admitted_bytes
        );
        assert_eq!(after.live_buffer_bytes, before.live_buffer_bytes);
        assert_eq!(after.peak_buffer_bytes, before.peak_buffer_bytes);
        let observed = ledger.snapshot();
        assert!(observed.complete);
        assert_eq!(observed.first_limit_call_ordinal, Some(257));
        assert_eq!(
            observed.first_limit_relation,
            RegistryLimitRelation::NewDistinctUnretained {
                unique_ordinal: 257
            }
        );
    }
    let observed = ledger.snapshot();
    assert_eq!(
        (
            observed.attempted_entries,
            observed.refused_entries,
            observed.sticky_refused_entries
        ),
        (258, 2, 1)
    );
    drop(ledger);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[test]
fn concurrent_registry_charge_creator_and_sticky_caller_preserve_exact_order() {
    let owner = scope(1);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let workers: Vec<_> = ["http://a.invalid/", "http://b.invalid/"]
        .into_iter()
        .map(|value| {
            let owner = owner.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                charge(&owner, &url(value))
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(SourceServiceError::Limit(SourceCounter::RegistryAttempts)))
            .count(),
        1
    );
    assert_eq!(
        charge(&owner, &url("http://later.invalid/")),
        Err(SourceServiceError::Limit(SourceCounter::RegistryAttempts))
    );
    let state = ledger.snapshot();
    assert!(state.complete);
    assert_eq!(
        (
            state.attempted_entries,
            state.admitted_entries,
            state.refused_entries,
            state.sticky_refused_entries
        ),
        (3, 1, 2, 1)
    );
    assert_eq!(state.first_limit_call_ordinal, Some(2));
    assert_eq!(
        state.first_limit_relation,
        RegistryLimitRelation::NewDistinctUnretained { unique_ordinal: 2 }
    );
}

#[test]
fn registry_identity_bounds_gaps_and_scalar_overflow_never_certify_partial_history() {
    let owner = scope(2);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    charge(&owner, &url(&"x".repeat(8192))).unwrap();
    assert!(ledger.snapshot().complete);
    charge(&owner, &url(&"y".repeat(8193))).unwrap(); // Original charge result unchanged.
    assert!(!ledger.snapshot().complete);
    assert_eq!(
        charge(&owner, &url("new")),
        Err(SourceServiceError::Limit(SourceCounter::RegistryAttempts))
    );
    assert_eq!(
        ledger.snapshot().first_limit_relation,
        RegistryLimitRelation::Unknown
    );
    assert_eq!(ledger.snapshot().retained_rows, 1);
    let owner = scope(3);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    let name = url("same");
    let exact = Path::new(&"p".repeat(8192)).to_path_buf();
    owner
        .charge_registry_request_for_test(RegistryRequestRef::Local {
            url: &name,
            path: &exact,
        })
        .unwrap();
    assert!(ledger.snapshot().complete);
    let oversized = Path::new(&"p".repeat(8193)).to_path_buf();
    owner
        .charge_registry_request_for_test(RegistryRequestRef::Local {
            url: &name,
            path: &oversized,
        })
        .unwrap();
    charge(&owner, &name).unwrap();
    assert_eq!(
        charge(&owner, &name),
        Err(SourceServiceError::Limit(SourceCounter::RegistryAttempts))
    );
    assert!(!ledger.snapshot().complete);
    assert_eq!(
        ledger.snapshot().first_limit_relation,
        RegistryLimitRelation::Unknown
    );
    let owner = scope(2);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    owner.charge(SourceCounter::RegistryAttempts, 1).unwrap(); // Unobserved prior charge.
    assert!(!ledger.snapshot().complete); // Also detects a trailing gap before another call.
    charge(&owner, &name).unwrap();
    assert!(charge(&owner, &name).is_err());
    assert_eq!(
        ledger.snapshot().first_limit_relation,
        RegistryLimitRelation::Unknown
    );
    let owner = scope(1);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    ledger.state.lock().unwrap().snapshot.attempted_entries = u64::MAX;
    assert_eq!(charge(&owner, &name), Ok(()));
    assert!(!ledger.snapshot().complete);
    assert_eq!(owner.snapshot().registry_attempts, 1);
    let owner = scope(1);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    ledger.state.lock().unwrap().snapshot.admitted_distinct = u64::MAX;
    assert_eq!(charge(&owner, &name), Ok(()));
    assert!(charge(&owner, &name).is_err());
    assert!(!ledger.snapshot().complete);
    assert_eq!(
        ledger.snapshot().first_limit_relation,
        RegistryLimitRelation::Unknown
    );
}

#[test]
fn registry_association_credit_pressure_weak_attachment_and_clone_last_drop() {
    let deadline = Instant::now() + Duration::from_secs(2);
    for limit in [128, 256] {
        let expected = limit as usize * ROW_BYTES
            + 2 * std::mem::size_of::<RegistryRequestAssociation>()
            + std::mem::size_of::<SourceTimeoutState>()
            + std::mem::size_of::<SourceBufferState>()
            + 64;
        for capacity in [expected - 1, expected] {
            let owner = SourceServiceScope::new_for_test(
                NormalizedAbsolutePath::new("/profile-credit").unwrap(),
                deadline,
                deadline + Duration::from_secs(1),
                SourceServiceLimits {
                    registry_attempts: limit,
                    live_buffer_bytes: capacity,
                    ..SourceServiceLimits::default()
                },
            )
            .unwrap();
            let armed = owner.arm_registry_association_for_test();
            if capacity < expected {
                assert!(matches!(armed, Err(SourceServiceError::BufferLimit)));
                assert_eq!(owner.snapshot().live_buffer_bytes, 0);
                continue;
            }
            let ledger = armed.unwrap();
            assert_eq!(ledger.snapshot().charged_bytes, expected);
            assert_eq!(
                ledger.state.lock().unwrap().slab.len(),
                limit as usize * ROW_BYTES
            );
            assert_eq!(owner.snapshot().live_buffer_bytes, expected);
            let weak = Arc::downgrade(&ledger);
            let clone = ledger.clone();
            drop(ledger);
            assert_eq!(owner.snapshot().live_buffer_bytes, expected);
            drop(clone);
            assert!(weak.upgrade().is_none());
            assert_eq!(owner.snapshot().live_buffer_bytes, 0);
            assert!(!owner.registry_observation_for_test().1.armed);
        }
    }
    let limits = SourceServiceLimits {
        live_buffer_bytes: 1024,
        ..SourceServiceLimits::default()
    };
    let owner = SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/pressure").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap();
    assert!(matches!(
        owner.arm_registry_association_for_test(),
        Err(SourceServiceError::BufferLimit)
    ));
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let owner = scope(1);
    let result = owner.arm_registry_association_with_hook(|| {
        owner.charge(SourceCounter::RegistryAttempts, 1).unwrap();
    });
    assert!(matches!(result, Err(SourceServiceError::ForeignScope)));
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let owner = scope(1);
    let result = owner.arm_registry_association_with_hook(|| owner.cancel());
    assert!(matches!(result, Err(SourceServiceError::Cancelled)));
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let owner = scope(1);
    let weak = Arc::downgrade(&owner);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    let clone = ledger.clone();
    let charged = ledger.snapshot().charged_bytes;
    assert!(charged > ROW_BYTES);
    assert_eq!(owner.snapshot().live_buffer_bytes, charged);
    drop(ledger);
    drop(owner);
    assert!(weak.upgrade().is_some());
    drop(clone);
    assert!(weak.upgrade().is_none());
    let owner = scope(1);
    let ledger = owner.arm_registry_association_for_test().unwrap();
    let ledger_weak = Arc::downgrade(&ledger);
    drop(ledger);
    assert!(ledger_weak.upgrade().is_none());
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert_eq!(charge(&owner, &url("unavailable")), Ok(()));
    assert!(matches!(
        owner.arm_registry_association_for_test(),
        Err(SourceServiceError::ForeignScope)
    ));
    let owner = scope(1);
    let barrier = Arc::new(Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let owner = owner.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                owner.arm_registry_association_for_test()
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(SourceServiceError::ForeignScope)))
            .count(),
        1
    );
    drop(results);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}
