//! Ordinary FD-kernel tests. An expected Arc here is NOT native eligibility or
//! custody; the separate actual sealed-root tests prove the closed caller gate.
use std::cell::Cell;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;

use sha2::Digest;
use sha2::Sha256;

use super::tests::anchor;
use super::tests::demand;
use super::tests::limits;
use super::tests::scope;
use super::*;
use crate::runtime::dice::source_service::HOST_FILE_OBJECT_BYTES;
use crate::runtime::dice::source_service::SourceServiceLimits;

struct Fixture {
    directory: tempfile::TempDir,
    descriptor: File,
    body: Arc<[u8]>,
}
impl Fixture {
    fn new(body: Arc<[u8]>) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("file"), body.as_ref()).unwrap();
        fs::set_permissions(root.join("file"), fs::Permissions::from_mode(0o644)).unwrap();
        let descriptor = File::open(&root).unwrap();
        Self {
            directory,
            descriptor,
            body,
        }
    }
    fn path(&self) -> std::path::PathBuf {
        self.directory.path().join("root/file")
    }
    fn root(&self) -> RootIo<'_> {
        RootIo::new(&self.descriptor, limits()).unwrap()
    }
    fn verify(
        &self,
        operation: PathObservationOperation,
        resources: &Arc<SourceServiceScope>,
        active: &dyn Fn() -> bool,
    ) -> Result<PathObservationResult, RootedObservationError> {
        // Match the observer's modeled scratch allowance. Actual descriptors
        // belong to this ordinary fixture, not to a manufactured native root.
        let _scratch = reserve(resources, NATIVE_CREDIT)?;
        owned::verify(
            &self.root(),
            Path::new("file"),
            &slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(self.body.clone()),
            0o644,
            operation,
            resources,
            active,
        )
    }
    fn digest(&self) -> FileContentDigest {
        FileContentDigest::new(
            Sha256::digest(self.body.as_ref()).into(),
            self.body.len() as u64,
        )
        .unwrap()
    }
}

#[test]
fn owned_kernel_verifies_payload_and_empty_eof_reuses_arc_and_charges_actual_bytes() {
    for body in [
        Arc::<[u8]>::from([]),
        Arc::from(b"A\0\xffowned\n".as_slice()),
    ] {
        let fixture = Fixture::new(body);
        let resources = scope(SourceServiceLimits::default());
        let bytes = fixture
            .verify(PathObservationOperation::FileBytes, &resources, &|| true)
            .unwrap();
        let PathObservationResult::FileBytes(PathOperationResult::Present(actual)) = bytes else {
            panic!("verified bytes expected")
        };
        assert!(Arc::ptr_eq(&actual, &fixture.body));
        assert_eq!(actual.as_ref(), fixture.body.as_ref());
        let digest = fixture
            .verify(PathObservationOperation::FileDigest, &resources, &|| true)
            .unwrap();
        assert!(
            matches!(digest, PathObservationResult::FileDigest(PathOperationResult::Present(value)) if value == fixture.digest())
        );
        // Fresh verification is repeated; this is not an observation cache.
        drop(
            fixture
                .verify(PathObservationOperation::FileBytes, &resources, &|| true)
                .unwrap(),
        );
        let counters = resources.snapshot();
        assert_eq!(counters.source_attempts, 0);
        assert_eq!(counters.decoded_bytes, 3 * fixture.body.len() as u64);
        assert_eq!(counters.live_buffer_bytes, 0);
        assert_eq!(resources.check(), Ok(()));
    }
}

#[test]
fn owned_kernel_refuses_actual_byte_size_name_type_mode_and_link_count_drift_without_capture_fallback()
 {
    for mutation in [
        "bytes",
        "truncate",
        "missing",
        "symlink",
        "directory",
        "mode",
        "hardlink",
    ] {
        for operation in [
            PathObservationOperation::FileBytes,
            PathObservationOperation::FileDigest,
        ] {
            let fixture = Fixture::new(Arc::from(b"abc".as_slice()));
            let resources = scope(SourceServiceLimits::default());
            match mutation {
                "bytes" => fs::write(fixture.path(), b"bad").unwrap(),
                "truncate" => File::options()
                    .write(true)
                    .open(fixture.path())
                    .unwrap()
                    .set_len(2)
                    .unwrap(),
                "missing" => fs::remove_file(fixture.path()).unwrap(),
                "symlink" => {
                    let outside = fixture.directory.path().join("outside");
                    fs::write(&outside, b"abc").unwrap();
                    fs::remove_file(fixture.path()).unwrap();
                    symlink(outside, fixture.path()).unwrap();
                }
                "directory" => {
                    fs::remove_file(fixture.path()).unwrap();
                    fs::create_dir(fixture.path()).unwrap();
                }
                "mode" => {
                    fs::set_permissions(fixture.path(), fs::Permissions::from_mode(0o600)).unwrap()
                }
                "hardlink" => {
                    fs::hard_link(fixture.path(), fixture.directory.path().join("alias")).unwrap()
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    fixture.verify(operation.clone(), &resources, &|| true),
                    Err(RootedObservationError::Unstable)
                ),
                "mutation={mutation}, operation={operation:?}"
            );
            let counters = resources.snapshot();
            assert_eq!(
                counters.source_attempts, 0,
                "eligible verification must never fall back to acquisition"
            );
            assert_eq!(
                counters.decoded_bytes,
                if mutation == "bytes" { 3 } else { 0 }
            );
            assert_eq!(counters.live_buffer_bytes, 0);
        }
    }
}

#[test]
fn owned_kernel_preserves_filebytes_exact_limit_and_larger_digest_bound() {
    let exact = Fixture::new(vec![b'x'; HOST_FILE_OBJECT_BYTES].into());
    let resources = scope(SourceServiceLimits::default());
    let bytes = exact
        .verify(PathObservationOperation::FileBytes, &resources, &|| true)
        .unwrap();
    assert!(
        matches!(bytes, PathObservationResult::FileBytes(PathOperationResult::Present(body)) if Arc::ptr_eq(&body, &exact.body))
    );
    assert_eq!(
        resources.snapshot().decoded_bytes,
        HOST_FILE_OBJECT_BYTES as u64
    );
    assert_eq!(resources.snapshot().source_attempts, 0);
    let larger = Fixture::new(vec![b'x'; HOST_FILE_OBJECT_BYTES + 1].into());
    let refused = scope(SourceServiceLimits::default());
    assert!(matches!(
        larger.verify(PathObservationOperation::FileBytes, &refused, &|| true),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes)
        ))
    ));
    assert_eq!(refused.snapshot().decoded_bytes, 0);
    assert_eq!(refused.snapshot().source_attempts, 0);
    assert_eq!(
        refused.check(),
        Err(SourceServiceError::Limit(SourceCounter::DecodedBytes))
    );
    assert_eq!(refused.snapshot().live_buffer_bytes, 0);
    let digest_scope = scope(SourceServiceLimits::default());
    let digest = larger
        .verify(PathObservationOperation::FileDigest, &digest_scope, &|| {
            true
        })
        .unwrap();
    assert!(
        matches!(digest, PathObservationResult::FileDigest(PathOperationResult::Present(value)) if value == larger.digest())
    );
    assert_eq!(
        digest_scope.snapshot().decoded_bytes,
        larger.body.len() as u64
    );
    assert_eq!(digest_scope.snapshot().source_attempts, 0);
    assert_eq!(digest_scope.snapshot().live_buffer_bytes, 0);
}

#[test]
fn owned_kernel_decoded_budget_is_cumulative_and_refuses_mid_read_without_publication() {
    let fixture = Fixture::new(vec![b'd'; 128 * 1024 + 1].into());
    let exact = scope(SourceServiceLimits {
        decoded_bytes: fixture.body.len() as u64,
        ..SourceServiceLimits::default()
    });
    fixture
        .verify(PathObservationOperation::FileDigest, &exact, &|| true)
        .unwrap();
    assert_eq!(exact.snapshot().decoded_bytes, fixture.body.len() as u64);
    assert!(matches!(
        fixture.verify(PathObservationOperation::FileDigest, &exact, &|| true),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes)
        ))
    ));
    assert_eq!(exact.snapshot().decoded_bytes, fixture.body.len() as u64);
    assert_eq!(exact.snapshot().source_attempts, 0);
    assert_eq!(exact.snapshot().live_buffer_bytes, 0);
    let partial = scope(SourceServiceLimits {
        decoded_bytes: 64 * 1024,
        ..SourceServiceLimits::default()
    });
    assert!(matches!(
        fixture.verify(PathObservationOperation::FileBytes, &partial, &|| true),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::DecodedBytes)
        ))
    ));
    assert!(partial.snapshot().decoded_bytes > 0);
    assert!(partial.snapshot().decoded_bytes <= 64 * 1024);
    assert_eq!(partial.snapshot().source_attempts, 0);
    assert_eq!(partial.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        partial.check(),
        Err(SourceServiceError::Limit(SourceCounter::DecodedBytes))
    );
}

#[test]
fn owned_kernel_checks_cancellation_between_actual_payload_chunks() {
    let fixture = Fixture::new(vec![b'c'; 256 * 1024].into());
    let resources = scope(SourceServiceLimits::default());
    let stopped = Cell::new(false);
    let active = || {
        if resources.snapshot().decoded_bytes > 0 {
            stopped.set(true);
            resources.cancel();
        }
        true
    };
    assert!(matches!(
        fixture.verify(PathObservationOperation::FileDigest, &resources, &active),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    assert!(stopped.get());
    assert!(resources.snapshot().decoded_bytes > 0);
    assert!(resources.snapshot().decoded_bytes < fixture.body.len() as u64);
    assert_eq!(resources.snapshot().source_attempts, 0);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
}

#[test]
fn owned_kernel_does_not_bypass_sticky_acquisition_refusal() {
    let fixture = Fixture::new(Arc::from(b"abc".as_slice()));
    let root = fixture.root();
    // The unchanged generic rooted test owns the exact64/+1 discriminator.
    // A tiny lower limit suffices to create the real sticky error here.
    let resources = scope(SourceServiceLimits {
        source_attempts: 1,
        ..SourceServiceLimits::default()
    });
    let selected = anchor(&fixture.directory.path().join("root"), 1);
    let request = demand(&selected, "file", PathObservationOperation::FileBytes);
    observe_rooted(&root, &selected, &[request.clone()], &resources, &|| true).unwrap();
    assert_eq!(resources.snapshot().source_attempts, 1);
    // Reaching the limit is not poison. An owned verification spends no extra
    // capture attempt, while still charging its actual payload work.
    fixture
        .verify(PathObservationOperation::FileBytes, &resources, &|| true)
        .unwrap();
    assert_eq!(resources.snapshot().source_attempts, 1);
    assert_eq!(
        resources.snapshot().decoded_bytes,
        2 * fixture.body.len() as u64
    );
    assert!(matches!(
        observe_rooted(&root, &selected, &[request], &resources, &|| true),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::SourceAttempts)
        ))
    ));
    let before = resources.snapshot();
    let checks = Cell::new(0usize);
    // Call the kernel directly: a wrapper's failed scratch reservation must not
    // masquerade as proof of the verifier's own poisoned-scope check.
    assert!(matches!(
        owned::verify(
            &root,
            Path::new("file"),
            &slug_bzlmod_v2::RepositoryWorkspaceBody::from_arc(fixture.body.clone()),
            0o644,
            PathObservationOperation::FileBytes,
            &resources,
            &|| {
                checks.set(checks.get() + 1);
                true
            }
        ),
        Err(RootedObservationError::SourceService(
            SourceServiceError::Limit(SourceCounter::SourceAttempts)
        ))
    ));
    assert_eq!(checks.get(), 0);
    assert_eq!(resources.snapshot().source_attempts, before.source_attempts);
    assert_eq!(resources.snapshot().decoded_bytes, before.decoded_bytes);
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
}

#[test]
fn packed_owned_kernel_materializes_only_file_bytes_with_independent_history_charge() {
    // Ordinary expected-body kernel evidence only; no native eligibility is minted.
    let fixture = Fixture::new(vec![0x5a; 128 * 1024 + 7].into());
    let mut builder =
        slug_bzlmod_v2::RepositoryWorkspaceBodyBuilder::new(fixture.body.len()).unwrap();
    for chunk in fixture.body.chunks(64 * 1024) {
        builder.append(chunk, &mut |_| Ok::<_, ()>(())).unwrap();
    }
    let expected = builder.finish().unwrap();
    assert!(expected.raw_arc().is_none());
    let expected_weak = expected.downgrade();
    let resources = scope(SourceServiceLimits::default());
    let bytes = owned::verify(
        &fixture.root(),
        Path::new("file"),
        &expected,
        0o644,
        PathObservationOperation::FileBytes,
        &resources,
        &|| true,
    )
    .unwrap();
    let PathObservationResult::FileBytes(PathOperationResult::Present(actual)) = bytes else {
        panic!("verified materialized bytes expected")
    };
    assert_eq!(actual.as_ref(), fixture.body.as_ref());
    assert!(!Arc::ptr_eq(&actual, &fixture.body));
    let weak_returned = Arc::downgrade(&actual);
    let digest = owned::verify(
        &fixture.root(),
        Path::new("file"),
        &expected,
        0o644,
        PathObservationOperation::FileDigest,
        &resources,
        &|| true,
    )
    .unwrap();
    assert!(
        matches!(digest, PathObservationResult::FileDigest(PathOperationResult::Present(value)) if value == fixture.digest())
    );
    assert_eq!(resources.snapshot().source_attempts, 0);
    assert_eq!(
        resources.snapshot().decoded_bytes,
        2 * fixture.body.len() as u64
    );
    assert_eq!(resources.snapshot().live_buffer_bytes, 0);
    drop(expected);
    assert!(expected_weak.upgrade().is_none());
    assert!(weak_returned.upgrade().is_some());
    let charged = resources.snapshot().decoded_bytes;
    drop(actual);
    assert!(weak_returned.upgrade().is_none());
    assert_eq!(resources.snapshot().decoded_bytes, charged);
}
