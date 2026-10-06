use super::*;

#[test]
fn io_projection_retains_kind_and_optional_signed_errno_without_message() {
    let error = io::Error::new(io::ErrorKind::PermissionDenied, "secret-one");
    let same = io::Error::new(io::ErrorKind::PermissionDenied, "secret-two");
    let left = SourceTransportFailure::io(RegistryRefusalSite::Dns, &error, "existing message");
    let right = SourceTransportFailure::io(RegistryRefusalSite::Dns, &same, "different message");
    assert_eq!(left.cause(), right.cause());
    assert_eq!(
        left.cause().detail,
        RegistryRefusalDetail::Io {
            kind: RegistryRefusalIoKind::PermissionDenied,
            errno: None
        }
    );
    assert!(!format!("{left:?}").contains("existing message"));
    assert!(!format!("{left:?}").contains("secret"));
    assert_eq!(left.into_message(), "existing message");
    for errno in [0, -1, i32::MIN, libc::ECONNREFUSED] {
        let error = io::Error::from_raw_os_error(errno);
        assert!(
            matches!(io_detail(&error), RegistryRefusalDetail::Io { errno: Some(actual), .. } if actual == errno)
        );
    }
    let io = io::Error::from_raw_os_error(libc::ECONNREFUSED);
    let native = SourceTransportFailure::io(
        RegistryRefusalSite::ConnectExhausted,
        &io,
        "source TCP connection failed",
    )
    .into_registry();
    assert_eq!(native.message, "source TCP connection failed");
    assert_eq!(
        native.cause.unwrap(),
        RegistryRefusalCause {
            site: RegistryRefusalSite::ConnectExhausted,
            detail: RegistryRefusalDetail::Io {
                kind: RegistryRefusalIoKind::ConnectionRefused,
                errno: Some(libc::ECONNREFUSED)
            }
        }
    );
}

#[test]
fn scope_projection_is_total_and_retains_exact_counter_and_message() {
    for (error, expected) in [
        (
            SourceServiceError::InvalidLimits,
            RegistryRefusalScopeFailure::InvalidLimits,
        ),
        (
            SourceServiceError::ForeignScope,
            RegistryRefusalScopeFailure::ForeignScope,
        ),
        (
            SourceServiceError::AcquisitionClosed,
            RegistryRefusalScopeFailure::AcquisitionClosed,
        ),
        (
            SourceServiceError::Cancelled,
            RegistryRefusalScopeFailure::Cancelled,
        ),
        (
            SourceServiceError::WorkDeadline,
            RegistryRefusalScopeFailure::WorkDeadline,
        ),
        (
            SourceServiceError::CleanupDeadline,
            RegistryRefusalScopeFailure::CleanupDeadline,
        ),
        (
            SourceServiceError::BufferLimit,
            RegistryRefusalScopeFailure::BufferLimit,
        ),
        (
            SourceServiceError::Allocation,
            RegistryRefusalScopeFailure::Allocation,
        ),
        (
            SourceServiceError::UnsupportedSourceOwner,
            RegistryRefusalScopeFailure::UnsupportedSourceOwner,
        ),
        (
            SourceServiceError::NetworkDisabled,
            RegistryRefusalScopeFailure::NetworkDisabled,
        ),
    ] {
        let failure = SourceTransportFailure::scope(error).into_registry();
        assert_eq!(failure.message.as_str(), error.to_string());
        assert_eq!(
            failure.cause,
            Some(RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(expected)
            })
        );
    }
    for (input, output) in [
        (
            SourceCounter::RegistryAttempts,
            RegistryRefusalScopeCounter::RegistryAttempts,
        ),
        (
            SourceCounter::RegistryBytes,
            RegistryRefusalScopeCounter::RegistryBytes,
        ),
        (
            SourceCounter::SourceAttempts,
            RegistryRefusalScopeCounter::SourceAttempts,
        ),
        (
            SourceCounter::TransferBytes,
            RegistryRefusalScopeCounter::TransferBytes,
        ),
        (
            SourceCounter::DecodedBytes,
            RegistryRefusalScopeCounter::DecodedBytes,
        ),
        (
            SourceCounter::WrittenBytes,
            RegistryRefusalScopeCounter::WrittenBytes,
        ),
    ] {
        for (error, expected) in [
            (
                SourceServiceError::Limit(input),
                RegistryRefusalScopeFailure::Limit(output),
            ),
            (
                SourceServiceError::Overflow(input),
                RegistryRefusalScopeFailure::Overflow(output),
            ),
        ] {
            assert_eq!(
                SourceTransportFailure::scope(error).cause().detail,
                RegistryRefusalDetail::Scope(expected)
            );
        }
    }
}
