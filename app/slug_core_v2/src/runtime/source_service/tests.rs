use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use slug_bzlmod_v2::RegistryFileUrl;
use slug_bzlmod_v2::RegistryIo;
use slug_bzlmod_v2::RegistryIoOutcome;
use slug_workspace_v2::NormalizedAbsolutePath;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use super::*;

fn scope(limits: SourceServiceLimits) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new(
        NormalizedAbsolutePath::new("/source-service-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}

#[test]
#[cfg(target_os = "linux")]
fn source_scope_actual_root_profile_preserves_other_limits_and_finite_ceiling() {
    let defaults = SourceServiceLimits::default();
    assert_eq!(defaults.registry_attempts, 128);
    assert_eq!(defaults.live_buffer_bytes, 16 << 20);
    assert_eq!(defaults.blocking_threads, None);
    let profile = SourceServiceLimits::actual_root();
    assert_eq!(profile.registry_attempts, 256);
    assert_eq!(profile.live_buffer_bytes, 40 << 20);
    assert_eq!(profile.blocking_threads, Some(2));
    assert_eq!(
        profile,
        SourceServiceLimits {
            registry_attempts: 256,
            live_buffer_bytes: 40 << 20,
            blocking_threads: Some(2),
            ..defaults
        }
    );
    for limit in [1, 128, 129, 256] {
        let owner = scope(SourceServiceLimits {
            registry_attempts: limit,
            ..defaults
        });
        owner
            .charge(SourceCounter::RegistryAttempts, limit)
            .unwrap();
        assert_eq!(
            owner.charge(SourceCounter::RegistryAttempts, 1),
            Err(SourceServiceError::Limit(SourceCounter::RegistryAttempts))
        );
        assert_eq!(owner.snapshot().registry_attempts, limit);
    }
    type Setter = fn(&mut SourceServiceLimits, u64);
    let ceilings: [(u64, Setter); 7] = [
        (defaults.registry_object_bytes, |l, n| {
            l.registry_object_bytes = n
        }),
        (256, |l, n| l.registry_attempts = n),
        (defaults.registry_admitted_bytes, |l, n| {
            l.registry_admitted_bytes = n
        }),
        (defaults.source_attempts, |l, n| l.source_attempts = n),
        (defaults.transferred_bytes, |l, n| l.transferred_bytes = n),
        (defaults.decoded_bytes, |l, n| l.decoded_bytes = n),
        (defaults.written_bytes, |l, n| l.written_bytes = n),
    ];
    let deadline = Instant::now() + Duration::from_secs(2);
    let construct = |limits| {
        SourceServiceScope::new(
            NormalizedAbsolutePath::new("/profile-limit-test").unwrap(),
            deadline,
            deadline + Duration::from_secs(1),
            limits,
        )
    };
    for threads in [1, 2] {
        assert!(
            construct(SourceServiceLimits {
                blocking_threads: Some(threads),
                ..defaults
            })
            .is_ok()
        );
    }
    for threads in [0, 3, usize::MAX] {
        assert_eq!(
            construct(SourceServiceLimits {
                blocking_threads: Some(threads),
                ..defaults
            })
            .err(),
            Some(SourceServiceError::InvalidLimits)
        );
    }
    for (ceiling, set) in ceilings {
        let mut limits = defaults;
        set(&mut limits, ceiling);
        assert!(construct(limits).is_ok());
        for invalid in [0, ceiling + 1] {
            set(&mut limits, invalid);
            assert_eq!(
                construct(limits).err(),
                Some(SourceServiceError::InvalidLimits)
            );
        }
    }
    for allowance in [
        1,
        defaults.live_buffer_bytes,
        defaults.live_buffer_bytes + 1,
        profile.live_buffer_bytes,
    ] {
        assert!(
            construct(SourceServiceLimits {
                live_buffer_bytes: allowance,
                ..defaults
            })
            .is_ok()
        );
    }
    let owner = scope(profile);
    let credit = owner.reserve_buffer(profile.live_buffer_bytes).unwrap();
    assert_eq!(
        owner.snapshot().live_buffer_bytes,
        profile.live_buffer_bytes
    );
    assert_eq!(
        owner.snapshot().peak_buffer_bytes,
        profile.live_buffer_bytes
    );
    assert_eq!(
        owner.reserve_buffer(1).err(),
        Some(SourceServiceError::BufferLimit)
    );
    drop(credit);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        owner.reserve_buffer(0).err(),
        Some(SourceServiceError::BufferLimit)
    );
    for invalid in [0, SourceServiceLimits::MAX_LIVE_BUFFER_BYTES + 1] {
        assert_eq!(
            construct(SourceServiceLimits {
                live_buffer_bytes: invalid,
                ..defaults
            })
            .err(),
            Some(SourceServiceError::InvalidLimits)
        );
    }
}

#[test]
fn source_scope_inclusive_counters_first_poison_and_no_refund() {
    for counter in [
        SourceCounter::RegistryAttempts,
        SourceCounter::RegistryBytes,
        SourceCounter::SourceAttempts,
        SourceCounter::TransferBytes,
        SourceCounter::DecodedBytes,
        SourceCounter::WrittenBytes,
    ] {
        let owner = scope(SourceServiceLimits::default());
        let maximum = owner.remaining(counter).unwrap();
        owner.charge(counter, maximum).unwrap();
        assert_eq!(owner.remaining(counter).unwrap(), 0);
        let first = SourceServiceError::Limit(counter);
        assert_eq!(owner.charge(counter, 1), Err(first));
        assert_eq!(owner.poison(SourceServiceError::Allocation), first);
        assert_eq!(owner.check(), Err(first));
        assert_eq!(owner.charge(counter, 0), Err(first));
    }
    let owner = scope(SourceServiceLimits::default());
    owner.charge(SourceCounter::RegistryAttempts, 1).unwrap();
    assert_eq!(
        owner.charge(SourceCounter::RegistryAttempts, u64::MAX),
        Err(SourceServiceError::Overflow(
            SourceCounter::RegistryAttempts
        ))
    );
    assert_eq!(owner.snapshot().registry_attempts, 1);
}

#[test]
fn source_scope_buffer_peak_history_last_drop_and_closed_export() {
    let mut limits = SourceServiceLimits::default();
    limits.live_buffer_bytes = 32;
    let owner = scope(limits);
    let old = owner.reserve_buffer(16).unwrap();
    let conversion = owner.reserve_buffer(16).unwrap();
    assert_eq!(owner.snapshot().peak_buffer_bytes, 32);
    assert_eq!(
        owner.reserve_buffer(1).err(),
        Some(SourceServiceError::BufferLimit)
    );
    drop(old);
    assert_eq!(owner.snapshot().live_buffer_bytes, 16);
    drop(conversion);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert_eq!(owner.check(), Err(SourceServiceError::BufferLimit));
    let weak = Arc::downgrade(&owner);
    drop(owner);
    assert!(weak.upgrade().is_none());

    let owner = scope(SourceServiceLimits::default());
    let held = owner.reserve_buffer(16).unwrap();
    let weak = Arc::downgrade(&owner);
    drop(owner);
    assert!(weak.upgrade().is_some());
    drop(held);
    assert!(weak.upgrade().is_none());

    let owner = scope(SourceServiceLimits::default());
    let held = owner.reserve_buffer(16).unwrap();
    owner.close_acquisition().unwrap();
    owner.check_closed_export().unwrap();
    assert_eq!(
        owner.reserve_buffer(1).err(),
        Some(SourceServiceError::AcquisitionClosed)
    );
    drop(held);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[tokio::test]
async fn source_scope_watch_cancellation_and_single_cleanup_clock() {
    let owner = scope(SourceServiceLimits::default());
    let waiting = owner.during(Duration::from_secs(1), async {
        std::future::pending::<Result<(), SourceServiceError>>().await
    });
    let cancel = async {
        tokio::task::yield_now().await;
        owner.cancel();
    };
    let (result, ()) = tokio::join!(waiting, cancel);
    assert_eq!(result, Err(SourceServiceError::Cancelled));
    // Poison stops acquisition but grants only disposal, not success/export.
    owner.during_cleanup(async { Ok(()) }).await.unwrap();
    assert_eq!(
        owner.close_acquisition(),
        Err(SourceServiceError::Cancelled)
    );

    let now = Instant::now();
    let work = now + Duration::from_millis(10);
    let owner = SourceServiceScope::new(
        NormalizedAbsolutePath::new("/deadline").unwrap(),
        work,
        work + Duration::from_millis(10),
        SourceServiceLimits::default(),
    )
    .unwrap();
    assert_eq!(
        owner
            .during(Duration::from_secs(1), async {
                std::future::pending::<Result<(), SourceServiceError>>().await
            })
            .await,
        Err(SourceServiceError::WorkDeadline)
    );
    assert!(
        owner
            .during_cleanup(async {
                std::future::pending::<Result<(), SourceServiceError>>().await
            })
            .await
            .is_err()
    );
    assert!(Instant::now() >= owner.cleanup_deadline());
    assert_eq!(owner.check(), Err(SourceServiceError::WorkDeadline));
    // A stricter inherited step cannot report late success while the absolute
    // work deadline is still in the future.
    let owner = scope(SourceServiceLimits::default());
    assert_eq!(
        owner
            .during(Duration::from_millis(1), async {
                std::thread::sleep(Duration::from_millis(3));
                Ok(())
            })
            .await,
        Err(SourceServiceError::WorkDeadline)
    );
    assert!(Instant::now() < owner.work_deadline());
    assert_eq!(owner.check(), Err(SourceServiceError::WorkDeadline));
    // A ready Future can synchronously hold the executor past the clock.
    // Both ready success and an already-expired ready future must refuse.
    for expires_before_poll in [false, true] {
        let work = Instant::now() + Duration::from_millis(1);
        let owner = SourceServiceScope::new(
            NormalizedAbsolutePath::new("/cleanup-ready").unwrap(),
            work,
            work + Duration::from_millis(1),
            SourceServiceLimits::default(),
        )
        .unwrap();
        let polled = std::cell::Cell::new(false);
        if expires_before_poll {
            std::thread::sleep(Duration::from_millis(3));
        }
        let result = owner
            .during_cleanup(async {
                polled.set(true);
                std::thread::sleep(Duration::from_millis(3));
                Ok(())
            })
            .await;
        assert_eq!(result, Err(SourceServiceError::CleanupDeadline));
        assert_eq!(polled.get(), !expires_before_poll);
        assert_eq!(owner.check(), Err(SourceServiceError::CleanupDeadline));
    }
    let work = Instant::now() + Duration::from_millis(1);
    let owner = SourceServiceScope::new(
        NormalizedAbsolutePath::new("/cleanup-primary").unwrap(),
        work,
        work + Duration::from_millis(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    let result = owner
        .during_cleanup(async {
            std::thread::sleep(Duration::from_millis(3));
            Err::<(), _>(SourceServiceError::UnsupportedSourceOwner)
        })
        .await;
    assert_eq!(result, Err(SourceServiceError::UnsupportedSourceOwner));
    assert_eq!(owner.check(), Err(SourceServiceError::CleanupDeadline));
}

#[test]
fn source_scope_validates_finite_limits_and_url_before_io() {
    let mut limits = SourceServiceLimits::default();
    limits.registry_object_bytes = 0;
    let deadline = Instant::now() + Duration::from_secs(1);
    assert!(
        SourceServiceScope::new(
            NormalizedAbsolutePath::new("/x").unwrap(),
            deadline,
            deadline + Duration::from_secs(1),
            limits
        )
        .is_err()
    );
    let prefix = "http://example.invalid/";
    let exact = url::Url::parse(&format!("{prefix}{}", "x".repeat(8192 - prefix.len()))).unwrap();
    assert_eq!(exact.as_str().len(), 8192);
    assert!(super::http::validate_address(&exact, SourceHttpAdmission::Registry).is_ok());
    let over = url::Url::parse(&format!("{}x", exact.as_str())).unwrap();
    assert!(super::http::validate_address(&over, SourceHttpAdmission::Registry).is_err());
    assert!(super::http::validate_address(&exact, SourceHttpAdmission::Archive).is_err());
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], 1234));
    let owner = scope(SourceServiceLimits::default());
    assert_eq!(
        super::http::bounded_addresses(&owner, std::iter::repeat_n(address, 64))
            .unwrap()
            .len(),
        64
    );
    assert!(super::http::bounded_addresses(&owner, std::iter::repeat_n(address, 65)).is_err());
    assert_eq!(owner.check(), Err(SourceServiceError::BufferLimit));
}

async fn duplex_response(
    owner: &Arc<SourceServiceScope>,
    wire: &[u8],
) -> Result<RegistryIoOutcome, slug_bzlmod_v2::RegistryTransportError> {
    let (client, mut server) = tokio::io::duplex(4096);
    let url = url::Url::parse("http://test.invalid/exact?yes=1").unwrap();
    let consumer = async {
        let response = super::http::request_test_io(owner, &url, client)
            .await
            .unwrap();
        super::registry::read_response(owner, response).await
    };
    let server = async {
        let mut request = Vec::new();
        let mut byte = [0; 1];
        while !request.ends_with(b"\r\n\r\n") {
            server.read_exact(&mut byte).await.unwrap();
            request.push(byte[0]);
        }
        assert!(request.starts_with(b"GET /exact?yes=1 HTTP/1.1\r\n"));
        // Fragmented chunks exercise the actual manually-driven body stream.
        for part in wire.chunks(7) {
            if server.write_all(part).await.is_err() {
                break;
            }
        }
        let _ = server.shutdown().await;
    };
    tokio::join!(consumer, server).0
}

#[tokio::test]
async fn scoped_registry_owned_http_fragmented_body_status_and_object_limit() {
    let owner = scope(SourceServiceLimits::default());
    assert_eq!(
        duplex_response(
            &owner,
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n2\r\ncd\r\n0\r\n\r\n"
        )
        .await
        .unwrap(),
        RegistryIoOutcome::Found(Arc::from(&b"abcd"[..]))
    );
    assert_eq!(owner.snapshot().registry_admitted_bytes, 4);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let mut limits = SourceServiceLimits::default();
    limits.registry_object_bytes = 3;
    let owner = scope(limits);
    assert!(
        duplex_response(&owner, b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nabcd")
            .await
            .is_err()
    );
    assert_eq!(
        owner.check(),
        Err(SourceServiceError::Limit(SourceCounter::RegistryBytes))
    );
    assert_eq!(owner.snapshot().registry_admitted_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let mut limits = SourceServiceLimits::default();
    limits.registry_object_bytes = 3;
    let owner = scope(limits);
    assert!(
        duplex_response(
            &owner,
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nab\r\n2\r\ncd\r\n0\r\n\r\n"
        )
        .await
        .is_err()
    );
    assert_eq!(
        owner.check(),
        Err(SourceServiceError::Limit(SourceCounter::RegistryBytes))
    );
    assert!(owner.snapshot().registry_admitted_bytes <= 3);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    // Pressure at the actual Arc conversion peak, after valid body admission.
    let mut limits = SourceServiceLimits::default();
    limits.registry_object_bytes = 16;
    limits.live_buffer_bytes =
        64 * 1024 + 128 * 256 + 32 * 1024 + 16 + std::mem::size_of::<Vec<u8>>() + 4 + 64 - 1;
    let owner = scope(limits);
    assert!(
        duplex_response(&owner, b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nabcd")
            .await
            .is_err()
    );
    assert_eq!(owner.check(), Err(SourceServiceError::BufferLimit));
    assert_eq!(owner.snapshot().registry_admitted_bytes, 4);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let owner = scope(SourceServiceLimits::default());
    assert!(
        duplex_response(&owner, b"HTTP/1.1 503 Nope\r\nContent-Length: 0\r\n\r\n")
            .await
            .is_err()
    );
    assert!(owner.check().is_ok()); // Ordinary fallback error is not poison.
}

#[tokio::test]
async fn scoped_registry_404_huge_trickle_is_status_first_and_closes_socket() {
    let owner = scope(SourceServiceLimits::default());
    let (client, mut server) = tokio::io::duplex(1024);
    let url = url::Url::parse("http://test.invalid/notfound").unwrap();
    let consumer = async {
        let response = super::http::request_test_io(&owner, &url, client)
            .await
            .unwrap();
        assert_eq!(
            super::registry::read_response(&owner, response)
                .await
                .unwrap(),
            RegistryIoOutcome::NotFound
        );
    };
    let server = async {
        let mut request = [0; 1024];
        server.read(&mut request).await.unwrap();
        server
            // Hyper reserves u64::MAX sentinels and rejects them before the
            // registry sees status. 1TiB is a valid protocol length and still
            // vastly exceeds every registry/body/supervisor admission bound.
            .write_all(b"HTTP/1.1 404 Missing\r\nContent-Length: 1099511627776\r\n\r\n")
            .await
            .unwrap();
        // No body is sent. Disposal must close immediately instead of polling
        // the held connection to drain the announced response.
        let mut next = [0; 1];
        assert_eq!(server.read(&mut next).await.unwrap(), 0);
    };
    tokio::join!(consumer, server);
    assert_eq!(owner.snapshot().registry_admitted_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let owner = scope(SourceServiceLimits::default());
    let (client, mut server) = tokio::io::duplex(1024);
    let consumer = async {
        let response = super::http::request_test_io(&owner, &url, client)
            .await
            .unwrap();
        owner.cancel();
        assert!(
            super::registry::read_response(&owner, response)
                .await
                .is_err()
        );
    };
    let server = async {
        let mut request = [0; 1024];
        server.read(&mut request).await.unwrap();
        server
            .write_all(b"HTTP/1.1 404 Missing\r\nContent-Length: 10000000\r\n\r\n")
            .await
            .unwrap();
        let mut next = [0; 1];
        assert_eq!(server.read(&mut next).await.unwrap(), 0);
    };
    tokio::join!(consumer, server);
    assert_eq!(owner.check(), Err(SourceServiceError::Cancelled));
    assert_eq!(owner.snapshot().registry_admitted_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[tokio::test]
async fn scoped_http_header_cap_cancellation_and_buffer_disposal() {
    let url = url::Url::parse("http://test.invalid/headers").unwrap();
    for headers in [128, 129] {
        let owner = scope(SourceServiceLimits::default());
        let (client, mut server) = tokio::io::duplex(4096);
        let consumer = super::http::request_test_io(&owner, &url, client);
        let writer = async {
            let mut request = [0; 1024];
            server.read(&mut request).await.unwrap();
            let mut response = b"HTTP/1.1 200 OK\r\n".to_vec();
            // Content-Length is also one header: exact128 vs one-over129.
            for _ in 0..headers - 1 {
                response.extend_from_slice(b"x: y\r\n");
            }
            response.extend_from_slice(b"Content-Length: 0\r\n\r\n");
            let _ = server.write_all(&response).await;
        };
        let result = tokio::join!(consumer, writer).0;
        if headers == 128 {
            let mut response = result.unwrap();
            assert!(response.next_data().await.unwrap().is_none());
            response.dispose().await.unwrap();
            owner.check().unwrap();
        } else {
            assert!(result.is_err());
            assert_eq!(
                owner.check(),
                Err(SourceServiceError::UnsupportedSourceOwner)
            );
        }
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
    let malformed_owner = scope(SourceServiceLimits::default());
    let (client, mut server) = tokio::io::duplex(1024);
    let consumer = super::http::request_test_io(&malformed_owner, &url, client);
    let writer = async {
        let mut request = [0; 1024];
        server.read(&mut request).await.unwrap();
        let _ = server.write_all(b"NOT-HTTP\r\n\r\n").await;
    };
    assert!(tokio::join!(consumer, writer).0.is_err());
    assert_eq!(
        malformed_owner.check(),
        Err(SourceServiceError::UnsupportedSourceOwner)
    );
    assert_eq!(malformed_owner.snapshot().live_buffer_bytes, 0);
    let owner = scope(SourceServiceLimits::default());
    let (client, mut server) = tokio::io::duplex(1024);
    let consumer = super::http::request_test_io(&owner, &url, client);
    let canceller = async {
        let mut request = [0; 1024];
        server.read(&mut request).await.unwrap();
        owner.cancel();
    };
    assert!(tokio::join!(consumer, canceller).0.is_err());
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert_eq!(owner.check(), Err(SourceServiceError::Cancelled));
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn scoped_registry_pinned_non_utf8_symlink_regular_special_and_object_limit() {
    use std::os::unix::ffi::OsStringExt;
    let over_owner = scope(SourceServiceLimits::default());
    let over_adapter = ScopedRegistryIo::new(Arc::clone(&over_owner));
    let over = RegistryFileUrl::new(format!("http://example.invalid/{}", "x".repeat(8193)));
    assert!(over_adapter.read_exact(&over).await.is_err());
    assert_eq!(over_owner.check(), Err(SourceServiceError::BufferLimit));
    assert_eq!(over_owner.snapshot().live_buffer_bytes, 0);
    let directory = tempfile::tempdir().unwrap();
    let path = directory
        .path()
        .join(std::ffi::OsString::from_vec(b"MODULE-\xff.bazel".to_vec()));
    std::fs::write(&path, b"module").unwrap();
    let link = directory.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    let owner = scope(SourceServiceLimits::default());
    let adapter = ScopedRegistryIo::new(Arc::clone(&owner));
    let url = RegistryFileUrl::new("file:///display-only".to_owned());
    assert_eq!(
        adapter.read_local_exact(&url, &path).await.unwrap(),
        RegistryIoOutcome::Found(Arc::from(&b"module"[..]))
    );
    assert_eq!(
        adapter.read_local_exact(&url, &link).await.unwrap(),
        RegistryIoOutcome::Found(Arc::from(&b"module"[..]))
    );
    assert_eq!(
        adapter
            .read_local_exact(&url, &directory.path().join("missing"))
            .await
            .unwrap(),
        RegistryIoOutcome::NotFound
    );
    assert_eq!(owner.snapshot().registry_admitted_bytes, 12);
    assert_eq!(owner.snapshot().registry_attempts, 3);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert!(
        adapter
            .read_local_exact(&url, directory.path())
            .await
            .is_err()
    );
    assert_eq!(
        owner.check(),
        Err(SourceServiceError::UnsupportedSourceOwner)
    );
    let fifo = directory.path().join("fifo");
    use std::os::unix::ffi::OsStrExt;
    let cpath = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let fifo_owner = scope(SourceServiceLimits::default());
    assert!(
        ScopedRegistryIo::new(Arc::clone(&fifo_owner))
            .read_local_exact(&url, &fifo)
            .await
            .is_err()
    );
    assert_eq!(
        fifo_owner.check(),
        Err(SourceServiceError::UnsupportedSourceOwner)
    );
    assert_eq!(fifo_owner.snapshot().registry_admitted_bytes, 0);
    assert_eq!(fifo_owner.snapshot().live_buffer_bytes, 0);
    let mut limits = SourceServiceLimits::default();
    limits.registry_object_bytes = 5;
    let owner = scope(limits);
    let adapter = ScopedRegistryIo::new(Arc::clone(&owner));
    assert!(adapter.read_local_exact(&url, &path).await.is_err());
    assert_eq!(owner.snapshot().registry_admitted_bytes, 0);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[tokio::test]
async fn typed_registry_status_and_body_are_selected_without_string_matching() {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalSite;
    let owner = scope(SourceServiceLimits::default());
    let result = duplex_response(
        &owner,
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
    )
    .await
    .unwrap_err();
    assert_eq!(
        result.cause,
        Some(RegistryRefusalCause {
            site: RegistryRefusalSite::HttpStatus,
            detail: RegistryRefusalDetail::Status(503)
        })
    );
    assert_eq!(
        result.message,
        "scoped registry returned HTTP 503 Service Unavailable"
    );
    owner.check().unwrap();
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let owner = scope(SourceServiceLimits::default());
    assert_eq!(
        duplex_response(
            &owner,
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n"
        )
        .await
        .unwrap(),
        RegistryIoOutcome::NotFound
    );
    let owner = scope(SourceServiceLimits::default());
    let failure = duplex_response(
        &owner,
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nnot-a-chunk\r\n",
    )
    .await
    .unwrap_err();
    assert!(matches!(
        failure.cause,
        Some(RegistryRefusalCause {
            site: RegistryRefusalSite::HttpBody | RegistryRefusalSite::HttpConnection,
            ..
        })
    ));
    assert!(matches!(
        failure.cause.unwrap().detail,
        RegistryRefusalDetail::Io {
            kind: slug_bzlmod_v2::RegistryRefusalIoKind::InvalidInput,
            errno: None
        }
    ));
    owner.check().unwrap();
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[tokio::test]
async fn typed_http_parse_keeps_selected_sticky_scope_refusal() {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;
    let owner = scope(SourceServiceLimits::default());
    let (client, mut server) = tokio::io::duplex(1024);
    let url = url::Url::parse("http://test.invalid/secret-query?credential=sentinel").unwrap();
    let consumer = super::http::request_test_io_typed(&owner, &url, client);
    let writer = async {
        let mut request = [0; 1024];
        server.read(&mut request).await.unwrap();
        let _ = server.write_all(b"NOT-HTTP\r\n\r\n").await;
    };
    let failure = match tokio::join!(consumer, writer).0 {
        Err(error) => error,
        Ok(_) => panic!("malformed protocol was admitted"),
    };
    // Scope::during's established post-check selects its parse poison; do not
    // relabel that final error with the discarded inner Hyper failure.
    assert_eq!(
        failure.cause(),
        RegistryRefusalCause {
            site: RegistryRefusalSite::Scope,
            detail: RegistryRefusalDetail::Scope(
                RegistryRefusalScopeFailure::UnsupportedSourceOwner
            )
        }
    );
    assert!(!format!("{failure:?}").contains("sentinel"));
    assert!(!format!("{failure:?}").contains("test.invalid"));
    assert_eq!(
        owner.check(),
        Err(SourceServiceError::UnsupportedSourceOwner)
    );
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}

#[tokio::test]
async fn typed_status_primary_survives_cleanup_expiry_but_success_returns_cleanup() {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;
    for status in [503, 404] {
        let work = Instant::now() + Duration::from_millis(100);
        let cleanup = work + Duration::from_millis(20);
        let owner = SourceServiceScope::new(
            NormalizedAbsolutePath::new("/typed-cleanup").unwrap(),
            work,
            cleanup,
            SourceServiceLimits::default(),
        )
        .unwrap();
        let (client, mut server) = tokio::io::duplex(1024);
        let url = url::Url::parse("http://test.invalid/exact").unwrap();
        let consumer = super::http::request_test_io_typed(&owner, &url, client);
        let writer = async {
            let mut request = [0; 1024];
            server.read(&mut request).await.unwrap();
            server
                .write_all(
                    format!("HTTP/1.1 {status} Reason\r\nContent-Length: 0\r\n\r\n").as_bytes(),
                )
                .await
                .unwrap();
        };
        let response = match tokio::join!(consumer, writer).0 {
            Ok(response) => response,
            Err(error) => panic!("response setup failed: {error:?}"),
        };
        tokio::time::sleep_until(cleanup.into()).await;
        let error = super::registry::read_response(&owner, response)
            .await
            .unwrap_err();
        let expected = if status == 503 {
            RegistryRefusalCause {
                site: RegistryRefusalSite::HttpStatus,
                detail: RegistryRefusalDetail::Status(503),
            }
        } else {
            RegistryRefusalCause {
                site: RegistryRefusalSite::Scope,
                detail: RegistryRefusalDetail::Scope(RegistryRefusalScopeFailure::CleanupDeadline),
            }
        };
        assert_eq!(error.cause, Some(expected));
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn typed_local_refusal_uses_actual_io_and_keeps_missing_successful() {
    use slug_bzlmod_v2::RegistryRefusalCause;
    use slug_bzlmod_v2::RegistryRefusalDetail;
    use slug_bzlmod_v2::RegistryRefusalScopeFailure;
    use slug_bzlmod_v2::RegistryRefusalSite;
    let directory = tempfile::tempdir().unwrap();
    let leaf = directory.path().join("private-sentinel");
    std::fs::write(&leaf, b"content").unwrap();
    let owner = scope(SourceServiceLimits::default());
    let adapter = ScopedRegistryIo::new(Arc::clone(&owner));
    let url = RegistryFileUrl::new("file:///credential-sentinel");
    let error = adapter
        .read_local_exact(&url, &leaf.join("child"))
        .await
        .unwrap_err();
    assert!(matches!(
        error.cause,
        Some(RegistryRefusalCause {
            site: RegistryRefusalSite::LocalPin,
            detail: RegistryRefusalDetail::Io {
                errno: Some(libc::ENOTDIR),
                ..
            }
        })
    ));
    assert_eq!(error.message, "scoped registry inode pin failed");
    assert!(!format!("{:?}", error.cause).contains("sentinel"));
    assert_eq!(
        adapter
            .read_local_exact(&url, &directory.path().join("missing"))
            .await
            .unwrap(),
        RegistryIoOutcome::NotFound
    );
    let error = adapter
        .read_local_exact(&url, directory.path())
        .await
        .unwrap_err();
    assert_eq!(
        error.cause,
        Some(RegistryRefusalCause {
            site: RegistryRefusalSite::Scope,
            detail: RegistryRefusalDetail::Scope(
                RegistryRefusalScopeFailure::UnsupportedSourceOwner
            )
        })
    );
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
}
