use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use super::super::SourceServiceLimits;
use super::*;

fn scope(bytes: usize) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_for_test(
        slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        SourceServiceLimits {
            live_decoder_bytes: bytes,
            ..SourceServiceLimits::default()
        },
    )
    .unwrap()
}

#[test]
fn decoder_bank_is_independent_and_refuses_simultaneous_overcommit() {
    let scope = scope(SourceServiceLimits::MAX_LIVE_DECODER_BYTES);
    assert_eq!(scope.limits().live_buffer_bytes, 16 << 20);
    let io = scope.reserve_buffer(64 * 1024).unwrap();
    let decoder = scope
        .reserve_decoder(SourceServiceLimits::MAX_LIVE_DECODER_BYTES)
        .unwrap();
    assert_eq!(scope.snapshot().live_decoder_bytes, 136 << 20);
    assert_eq!(scope.snapshot().peak_decoder_bytes, 136 << 20);
    assert_eq!(scope.snapshot().live_buffer_bytes, 64 * 1024);
    assert!(matches!(
        scope.reserve_decoder(1),
        Err(SourceServiceError::DecoderLimit)
    ));
    assert_eq!(
        scope.first_error_for_test(),
        Some(SourceServiceError::DecoderLimit)
    );
    assert_eq!(
        scope.poison(SourceServiceError::Cancelled),
        SourceServiceError::DecoderLimit
    );
    drop(decoder);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
    assert_eq!(scope.snapshot().peak_decoder_bytes, 136 << 20);
    assert!(matches!(
        scope.reserve_decoder(1),
        Err(SourceServiceError::DecoderLimit)
    ));
    assert_eq!(scope.snapshot().live_buffer_bytes, 64 * 1024);
    drop(io);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
}

#[test]
fn decoder_credit_retires_on_success_failure_and_cancel_and_owns_scope() {
    for failure in [false, true] {
        let scope = scope(64);
        let run = || -> Result<(), &'static str> {
            let _decoder = scope.reserve_decoder(64).unwrap();
            assert_eq!(scope.snapshot().live_decoder_bytes, 64);
            if failure {
                return Err("codec failed");
            }
            Ok(())
        };
        assert_eq!(run().is_err(), failure);
        assert_eq!(scope.snapshot().live_decoder_bytes, 0);
        assert_eq!(scope.snapshot().peak_decoder_bytes, 64);
        assert_eq!(scope.snapshot().live_buffer_bytes, 0);
        assert!(scope.check().is_ok());
    }
    let scope = scope(64);
    let decoder = scope.reserve_decoder(64).unwrap();
    let weak = Arc::downgrade(&scope);
    scope.cancel();
    assert!(matches!(
        scope.reserve_decoder(1),
        Err(SourceServiceError::Cancelled)
    ));
    drop(decoder);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
    assert_eq!(scope.snapshot().peak_decoder_bytes, 64);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    drop(scope);
    assert!(weak.upgrade().is_none());

    let retained = SourceServiceScope::new_for_test(
        slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
        Instant::now() + Duration::from_secs(5),
        Instant::now() + Duration::from_secs(6),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let decoder = retained.reserve_decoder(64).unwrap();
    let weak = Arc::downgrade(&retained);
    drop(retained);
    assert!(
        weak.upgrade().is_some(),
        "credit owns its scope through cleanup"
    );
    drop(decoder);
    assert!(weak.upgrade().is_none());
}

#[test]
fn decoder_tiny_quota_refuses_before_consumer_and_validates_policy() {
    let scope = scope(1);
    let mut consumer_started = false;
    let result = scope.reserve_decoder(136 << 20).map(|_credit| {
        consumer_started = true;
    });
    assert!(matches!(result, Err(SourceServiceError::DecoderLimit)));
    assert!(!consumer_started);
    assert_eq!(scope.snapshot().live_decoder_bytes, 0);
    assert_eq!(scope.snapshot().peak_decoder_bytes, 0);
    assert_eq!(scope.snapshot().live_buffer_bytes, 0);
    for invalid in [0, SourceServiceLimits::MAX_LIVE_DECODER_BYTES + 1] {
        let deadline = Instant::now() + Duration::from_secs(5);
        assert!(matches!(
            SourceServiceScope::new_for_test(
                slug_workspace_v2::NormalizedAbsolutePath::new("/workspace").unwrap(),
                deadline,
                deadline + Duration::from_secs(1),
                SourceServiceLimits {
                    live_decoder_bytes: invalid,
                    ..SourceServiceLimits::default()
                },
            ),
            Err(SourceServiceError::InvalidLimits)
        ));
    }
}
