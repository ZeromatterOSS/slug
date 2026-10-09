use std::cell::Cell;
use std::io::Cursor;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::time::Duration;
use std::time::Instant;

use super::source_service::SourceCounter;
use super::source_service::SourceServiceError;
use super::source_service::SourceServiceScope;
use super::tests::begin_empty;
use super::tests::generated_plan;
use super::tests::materialization_request;
use super::tests::native_request;
use super::*;
use crate::runtime::dice::source_service::SourceServiceLimits;

fn scope(
    workspace: &NormalizedAbsolutePath,
    limits: SourceServiceLimits,
) -> Arc<SourceServiceScope> {
    let work = Instant::now() + Duration::from_secs(2);
    SourceServiceScope::new_for_test(
        workspace.clone(),
        work,
        work + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}

#[test]
#[cfg(target_os = "linux")]
fn scoped_native_origin_callback_runs_outside_session_lock_before_postcheck() {
    use std::os::unix::ffi::OsStrExt;

    let directory = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(directory.path().to_owned()).unwrap();
    let own = scope(&workspace, SourceServiceLimits::default());
    let materializer = RepositoryMaterializer::new_scoped(workspace, own.clone()).unwrap();
    let token = begin_empty(&materializer);
    let file = directory.path().join("body");
    std::fs::write(&file, b"abc").unwrap();
    let demand = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(file).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let calls = Cell::new(0);
    let observed = materializer
        .observe_native_with_charge_origin(token, [demand.clone()], &mut |_| {
            calls.set(calls.get() + 1)
        })
        .unwrap();
    assert!(matches!(observed.get(&demand).unwrap().as_ref(),
        PathObservationResult::FileBytes(PathOperationResult::Present(body)) if body.as_ref() == b"abc"));
    assert_eq!(calls.get(), 0);
    assert_eq!(own.snapshot().decoded_bytes, 3);
    own.charge(SourceCounter::SourceAttempts, 63).unwrap();
    let absent = PathObservationDemand::new(
        PathObservationNamespace::Host,
        NormalizedAbsolutePath::new(directory.path().join("absent")).unwrap(),
        PathObservationOperation::FileBytes,
    );
    let original_path = absent.path().as_path().as_os_str().as_bytes().as_ptr();
    let counters = |s: crate::runtime::dice::source_service::SourceServiceCounters| {
        (
            s.registry_attempts,
            s.registry_admitted_bytes,
            s.source_attempts,
            s.transferred_bytes,
            s.decoded_bytes,
            s.written_bytes,
            s.live_buffer_bytes,
            s.peak_buffer_bytes,
        )
    };
    let before = counters(own.snapshot());
    let error = RepositorySessionError::SourceService(SourceServiceError::Limit(
        SourceCounter::SourceAttempts,
    ));
    assert_eq!(
        materializer.observe_native_with_charge_origin(token, [absent.clone()], &mut |original| {
            calls.set(calls.get() + 1);
            assert_eq!(original, &absent);
            assert_eq!(
                original.path().as_path().as_os_str().as_bytes().as_ptr(),
                original_path
            );
            // Both reentries would deadlock if either observer lock survived.
            assert!(materializer.session_is_active(token));
            assert_eq!(counters(own.snapshot()), before);
            assert_eq!(
                own.check(),
                Err(SourceServiceError::Limit(SourceCounter::SourceAttempts))
            );
            assert!(!original.path().as_path().exists());
        },),
        Err(error)
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(counters(own.snapshot()), before);
    assert!(matches!(
        materializer.observe_native_with_charge_origin(token, [absent.clone()], &mut |_| calls
            .set(calls.get() + 1),),
        Err(RepositorySessionError::SourceService(
            SourceServiceError::Limit(SourceCounter::SourceAttempts)
        ))
    ));
    assert_eq!(calls.get(), 1);
    materializer.discard(token).unwrap();

    let fresh = scope(materializer.workspace(), SourceServiceLimits::default());
    let inactive =
        RepositoryMaterializer::new_scoped(materializer.workspace().clone(), fresh.clone())
            .unwrap();
    let stale = begin_empty(&inactive);
    inactive.discard(stale).unwrap();
    assert!(inactive.observe_native_with_charge_origin(
        stale, [absent], &mut |_| calls.set(calls.get() + 1),
    ).is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(fresh.snapshot().source_attempts, 0);

    // Enabled reporting borrows both owners and observes the same original error.
    let work = Instant::now() + Duration::from_secs(2);
    let census = SourceServiceScope::new_for_test_with_source_charge_census(
        materializer.workspace().clone(),
        work,
        work + Duration::from_secs(1),
        SourceServiceLimits {
            source_attempts: 1,
            ..SourceServiceLimits::default()
        },
        None,
    )
    .unwrap();
    let counted =
        RepositoryMaterializer::new_scoped(materializer.workspace().clone(), census.clone())
            .unwrap();
    let token = begin_empty(&counted);
    let first = Cell::new(0);
    let events = Cell::new(0);
    for expected in 1..=2 {
        let observed = counted.observe_native_with_charge_census(
            token,
            [demand.clone()],
            &mut |_| first.set(first.get() + 1),
            &mut |scope, original, record| {
                assert!(std::ptr::eq(scope, census.as_ref()));
                assert_eq!(original, &demand);
                assert_eq!(
                    original.path().as_path().as_os_str().as_bytes().as_ptr(),
                    demand.path().as_path().as_os_str().as_bytes().as_ptr()
                );
                assert!(counted.session_is_active(token));
                assert_eq!(scope.snapshot().source_attempts, 1);
                assert_eq!(record.first_created, expected == 2);
                assert_eq!(
                    (record.event, record.before, record.after),
                    (expected, if expected == 1 { 0 } else { 1 }, 1)
                );
                events.set(events.get() + 1);
                super::super::path_observation::report_unassociated_charge(scope, original, record);
            },
        );
        if expected == 1 {
            assert!(matches!(observed.unwrap().get(&demand).unwrap().as_ref(),
                PathObservationResult::FileBytes(PathOperationResult::Present(body)) if body.as_ref() == b"abc"));
        } else {
            assert_eq!(
                observed,
                Err(RepositorySessionError::SourceService(
                    SourceServiceError::Limit(SourceCounter::SourceAttempts)
                ))
            );
        }
    }
    assert_eq!((events.get(), first.get()), (2, 1));
    assert_eq!(census.snapshot().decoded_bytes, 3);
    assert!(
        counted
            .observe_native_with_charge_census(
                token,
                [demand],
                &mut |_| first.set(first.get() + 1),
                &mut |_, _, _| events.set(events.get() + 1)
            )
            .is_err()
    );
    assert_eq!((events.get(), first.get()), (2, 1));
    counted.discard(token).unwrap();
}

#[test]
fn scoped_materializer_foreign_missing_and_poisoned_cached_requests_do_not_call_source() {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let own = scope(&workspace, SourceServiceLimits::default());
    let foreign = scope(&workspace, SourceServiceLimits::default());
    let materializer = RepositoryMaterializer::new_scoped(workspace.clone(), own.clone()).unwrap();
    let token = begin_empty(&materializer);
    let request = materialization_request(
        &workspace,
        "repo",
        "one",
        RepositoryMaterializationKind::Immutable,
    );
    let called = Cell::new(0);
    for supplied in [None, Some(&foreign)] {
        let result = materializer.materialize_with_scope(
            token,
            request.clone(),
            RepositoryMaterializationGeneration(1),
            supplied,
            || {
                called.set(called.get() + 1);
                unreachable!()
            },
        );
        assert!(matches!(
            result,
            Err(RepositorySessionError::SourceService(
                SourceServiceError::ForeignScope
            ))
        ));
    }
    assert_eq!(called.get(), 0);
    foreign.check().unwrap();
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path().to_owned();
    materializer
        .materialize_with_scope(
            token,
            request.clone(),
            RepositoryMaterializationGeneration(1),
            Some(&own),
            || {
                called.set(called.get() + 1);
                RepositoryMaterializationAttempt::Immutable {
                    bytes: b"normal hash owner".to_vec(),
                    root,
                }
            },
        )
        .unwrap();
    assert_eq!(called.get(), 1);
    assert!(root_path.exists());
    materializer
        .materialize_with_scope(
            token,
            request.clone(),
            RepositoryMaterializationGeneration(2),
            Some(&own),
            || unreachable!("cached exact request must not reacquire"),
        )
        .unwrap();
    own.cancel();
    assert!(matches!(
        materializer.materialize_with_scope(
            token,
            request,
            RepositoryMaterializationGeneration(3),
            Some(&own),
            || unreachable!("poison must precede cached return")
        ),
        Err(RepositorySessionError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    materializer.discard(token).unwrap();
    assert!(!root_path.exists());
}

#[test]
fn scoped_late_valid_hash_cannot_allocate_instance_or_publish_and_drops_root() {
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    let own = scope(&workspace, SourceServiceLimits::default());
    let materializer = RepositoryMaterializer::new_scoped(workspace.clone(), own.clone()).unwrap();
    let token = begin_empty(&materializer);
    let request = materialization_request(
        &workspace,
        "repo",
        "one",
        RepositoryMaterializationKind::Immutable,
    );
    let root = tempfile::tempdir().unwrap();
    let path = root.path().to_owned();
    let result = materializer.materialize_with_scope(
        token,
        request,
        RepositoryMaterializationGeneration(1),
        Some(&own),
        || {
            own.cancel();
            RepositoryMaterializationAttempt::Immutable {
                bytes: b"complete SHA-bound bytes".to_vec(),
                root,
            }
        },
    );
    assert!(matches!(
        result,
        Err(RepositorySessionError::SourceService(
            SourceServiceError::Cancelled
        ))
    ));
    assert!(!path.exists());
    let state = materializer.state.lock().unwrap();
    assert_eq!(state.next_instance, 1);
    assert!(state.active.as_ref().unwrap().entries.is_empty());
    assert!(state.active.as_ref().unwrap().provisional_roots.is_empty());
    drop(state);
    materializer.discard(token).unwrap();
}

#[test]
fn scoped_native_gateway_counts_real_file_effects_and_refuses_git_before_fallback() {
    let root = tempfile::tempdir().unwrap();
    let workspace = NormalizedAbsolutePath::new(root.path().to_owned()).unwrap();
    let own = scope(&workspace, SourceServiceLimits::default());
    let materializer = RepositoryMaterializer::new_scoped(workspace.clone(), own.clone()).unwrap();
    let token = begin_empty(&materializer);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let plan = generated_plan([
        ("BUILD.bazel", b"one" as &[u8], false),
        ("bin/tool", b"two" as &[u8], true),
    ]);
    let spec = slug_bzlmod_v2::RepoSpec {
        rule_id: slug_bzlmod_v2::RepoRuleId {
            bzl_file: slug_identity_v2::CanonicalLabel::parse("@@extension+repo//:defs.bzl")
                .unwrap(),
            rule_name: "generated_repository".into(),
        },
        attributes: Arc::default(),
    };
    let request = native_request(
        &workspace,
        "generated",
        spec,
        RepositoryMaterializationKind::GeneratedFileEffects(plan),
    );
    materializer
        .materialize_native_with_runtime(
            token,
            request,
            RepositoryMaterializationGeneration(1),
            &runtime,
            Some(&own),
        )
        .unwrap();
    assert_eq!(own.snapshot().written_bytes, 6);
    let state = materializer.state.lock().unwrap();
    let retained = &state.active.as_ref().unwrap().provisional_roots[0].root;
    assert_eq!(
        std::fs::read(retained.path().join("bin/tool")).unwrap(),
        b"two"
    );
    drop(state);
    let before = own.snapshot();
    let git = materialization_request(
        &workspace,
        "git",
        "missing-never-opened",
        RepositoryMaterializationKind::Immutable,
    );
    let result = materializer.materialize_native_with_runtime(
        token,
        git,
        RepositoryMaterializationGeneration(2),
        &runtime,
        Some(&own),
    );
    assert!(matches!(
        result,
        Err(RepositorySessionError::SourceService(
            SourceServiceError::UnsupportedSourceOwner
        ))
    ));
    let after = own.snapshot();
    assert_eq!(after.source_attempts, before.source_attempts);
    assert_eq!(after.transferred_bytes, before.transferred_bytes);
    assert_eq!(after.written_bytes, before.written_bytes);
    assert_eq!(
        materializer
            .state
            .lock()
            .unwrap()
            .active
            .as_ref()
            .unwrap()
            .entries
            .len(),
        1
    );
    materializer.discard(token).unwrap();
}

#[test]
fn counted_reads_exact_eof_overflow_cancel_and_failed_writes_preserve_admission() {
    struct Recording {
        bytes: Cursor<Vec<u8>>,
        sizes: Vec<usize>,
    }
    impl Read for Recording {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            self.sizes.push(output.len());
            self.bytes.read(output)
        }
    }
    let workspace = NormalizedAbsolutePath::new("/workspace").unwrap();
    for extra in [false, true] {
        let own = scope(
            &workspace,
            SourceServiceLimits {
                decoded_bytes: 4,
                ..SourceServiceLimits::default()
            },
        );
        let mut input = Recording {
            bytes: Cursor::new(if extra {
                b"12345".to_vec()
            } else {
                b"1234".to_vec()
            }),
            sizes: Vec::new(),
        };
        let mut output = [0; 64];
        assert_eq!(
            source_service::read_counted(
                &mut input,
                &mut output,
                &|| true,
                Some(&own),
                SourceCounter::DecodedBytes
            )
            .unwrap(),
            4
        );
        assert_eq!(&output[..4], b"1234");
        let tail = source_service::read_counted(
            &mut input,
            &mut output,
            &|| true,
            Some(&own),
            SourceCounter::DecodedBytes,
        );
        assert_eq!(tail.is_err(), extra);
        assert_eq!(input.sizes, [4, 1]);
        assert_eq!(own.snapshot().decoded_bytes, 4);
        if extra {
            assert_eq!(
                own.check(),
                Err(SourceServiceError::Limit(SourceCounter::DecodedBytes))
            );
        }
    }
    let own = scope(&workspace, SourceServiceLimits::default());
    let mut input = Recording {
        bytes: Cursor::new(b"a".to_vec()),
        sizes: Vec::new(),
    };
    assert!(
        source_service::read_counted(
            &mut input,
            &mut [0; 4],
            &|| false,
            Some(&own),
            SourceCounter::DecodedBytes
        )
        .is_err()
    );
    assert!(input.sizes.is_empty());
    struct Failing(Vec<u8>);
    impl Write for Failing {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0.is_empty() {
                self.0.push(bytes[0]);
                Ok(1)
            } else {
                Err(io::Error::other("controlled partial write"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output = Failing(Vec::new());
    assert!(source_service::write_counted(&mut output, b"four", &|| true, Some(&own)).is_err());
    assert_eq!(output.0, b"f");
    assert_eq!(own.snapshot().written_bytes, 4);
}
