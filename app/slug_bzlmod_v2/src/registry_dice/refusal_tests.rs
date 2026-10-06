use super::*;

fn cause() -> RegistryRefusalCause {
    RegistryRefusalCause {
        site: RegistryRefusalSite::Dns,
        detail: RegistryRefusalDetail::Io {
            kind: RegistryRefusalIoKind::PermissionDenied,
            errno: Some(-1),
        },
    }
}

#[test]
fn cause_is_inline_and_scalar_codes_are_stable() {
    assert!(std::mem::size_of::<RegistryRefusalCause>() <= 64);
    assert_eq!(RegistryRefusalSite::AddressValidation.code(), 1);
    assert_eq!(RegistryRefusalSite::Dns.code(), 2);
    assert_eq!(RegistryRefusalSite::InternalState.code(), 21);
    assert_eq!(RegistryRefusalIoKind::NotFound.code(), 1);
    assert_eq!(RegistryRefusalIoKind::Unknown.code(), 21);
    assert_eq!(RegistryRefusalProtocolFailure::Parse.code(), 1);
    assert_eq!(RegistryRefusalProtocolFailure::Other.code(), 5);
    assert_eq!(RegistryRefusalScopeCounter::RegistryAttempts.code(), 1);
    assert_eq!(RegistryRefusalScopeCounter::WrittenBytes.code(), 6);
    assert_eq!(RegistryRefusalScopeFailure::InvalidLimits.code(), 1);
    assert_eq!(
        RegistryRefusalScopeFailure::UnsupportedSourceOwner.code(),
        11
    );
    assert_eq!(
        RegistryRefusalScopeFailure::Limit(RegistryRefusalScopeCounter::RegistryBytes).code(),
        7
    );
    assert_eq!(
        RegistryRefusalScopeFailure::Overflow(RegistryRefusalScopeCounter::WrittenBytes).counter(),
        Some(RegistryRefusalScopeCounter::WrittenBytes)
    );
    assert_eq!(RegistryRefusalScopeFailure::Cancelled.counter(), None);
    assert_eq!(RegistryRefusalScopeFailure::NetworkDisabled.code(), 12);
    assert_eq!(RegistryRefusalScopeFailure::NetworkDisabled.counter(), None);
}

#[test]
fn optional_cause_changes_equality_but_preserves_established_debug_bytes() {
    let url = RegistryFileUrl::new("https://example.invalid/file");
    let path = PathBuf::from("/registry/file");
    for cause in [None, Some(cause())] {
        let transport = RegistryTransportError {
            message: "offline".into(),
            cause,
        };
        assert_eq!(
            format!("{transport:?}"),
            "RegistryTransportError { message: \"offline\" }"
        );
        let remote = RegistryRemoteError::Transport {
            url: url.dupe(),
            message: "offline".into(),
            cause,
        };
        let local = RegistryLocalError::Read {
            url: url.dupe(),
            path: path.clone(),
            message: "offline".into(),
            cause,
        };
        let file_remote = RegistryFileError::from(remote.clone());
        let file_local = RegistryFileError::from(local.clone());
        assert_eq!(
            format!("{remote:?}"),
            format!("Transport {{ url: {url:?}, message: \"offline\" }}")
        );
        assert_eq!(format!("{file_remote:?}"), format!("{remote:?}"));
        assert_eq!(
            format!("{local:?}"),
            format!("Read {{ url: {url:?}, path: {path:?}, message: \"offline\" }}")
        );
        assert_eq!(
            format!("{file_local:?}"),
            format!("LocalRead {{ path: {path:?}, message: \"offline\" }}")
        );
        assert!(
            matches!(file_remote, RegistryFileError::Transport { cause: actual, .. } if actual == cause)
        );
        assert!(
            matches!(file_local, RegistryFileError::LocalRead { cause: actual, .. } if actual == cause)
        );
    }
    let none = RegistryTransportError {
        message: "same".into(),
        cause: None,
    };
    let typed = RegistryTransportError {
        message: "same".into(),
        cause: Some(cause()),
    };
    assert_ne!(none, typed);
    assert_eq!(format!("{none:#?}"), format!("{typed:#?}"));
    let left = Arc::new(Err(RegistryFileError::Transport {
        url: url.dupe(),
        message: "same".into(),
        cause: None,
    }));
    let right = Arc::new(Err(RegistryFileError::Transport {
        url,
        message: "same".into(),
        cause: Some(cause()),
    }));
    assert!(RegistryFileKey::validity(&left));
    assert!(!RegistryFileKey::equality(&left, &right));
}
