use std::sync::Arc;
use std::sync::Barrier;

use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::dice::source_service::RegistryRequestAssociation;
use crate::runtime::dice::source_service::SourceCounter;
use crate::runtime::dice::source_service::SourceServiceError;
use crate::runtime::dice::source_service::SourceServiceLimits;
use crate::runtime::dice::source_service::SourceServiceScope;

fn scope(work: Duration, limits: SourceServiceLimits) -> Arc<SourceServiceScope> {
    let deadline = Instant::now() + work;
    SourceServiceScope::new_for_test(
        NormalizedAbsolutePath::new("/timeout-provenance-test").unwrap(),
        deadline,
        deadline + Duration::from_secs(1),
        limits,
    )
    .unwrap()
}
fn armed(work: Duration) -> (Arc<SourceServiceScope>, Arc<RegistryRequestAssociation>) {
    let owner = scope(work, SourceServiceLimits::default());
    let ledger = owner.arm_registry_association_for_test().unwrap();
    (owner, ledger)
}
fn expire(bound: Instant) {
    while Instant::now() < bound {
        std::hint::spin_loop();
    }
}
fn origin(ledger: &RegistryRequestAssociation) -> SourceTimeoutOrigin {
    let snapshot = ledger.snapshot().timeout;
    assert!(snapshot.armed && snapshot.first_error_is_work_deadline);
    snapshot.origin.unwrap()
}

#[test]
fn generic_first_deadline_caller_is_scalar_exact_and_sticky() {
    for (file, owner) in [
        ("src/runtime/source_service/scope.rs", 1),
        ("src/runtime/source_service/regular_file.rs", 2),
        ("src/runtime/path_observation.rs", 3),
        ("src/runtime/path_observation/scoped_directory.rs", 4),
        ("src/runtime/dice.rs", 5),
        ("src/runtime/input_image_bridge.rs", 6),
        ("src/runtime/source_service/diagnostic.rs", 7),
        ("src/runtime/source_service/timeout_provenance/tests.rs", 8),
    ] {
        assert_eq!(
            SourceTimeoutCaller::project(file, 42),
            SourceTimeoutCaller { owner, line: 42 }
        );
    }
    assert_eq!(
        SourceTimeoutCaller::project("app/slug_core_v2/src/runtime/dice.rs", 42).owner,
        5
    );
    for file in [
        "scope.rs",
        "foreign/src/runtime/dice.rs",
        "/tmp/src/runtime/dice.rs",
    ] {
        assert_eq!(
            SourceTimeoutCaller::project(file, 42),
            SourceTimeoutCaller { owner: 0, line: 0 }
        );
    }
    let mut observed_lines = [0; 2];
    for (index, observed_line) in observed_lines.iter_mut().enumerate() {
        let (owner, ledger) = armed(Duration::from_millis(10));
        expire(owner.work_deadline());
        let (expected_line, result) = if index == 0 {
            (line!(), owner.check())
        } else {
            (line!(), owner.check())
        };
        assert_eq!(result, Err(SourceServiceError::WorkDeadline));
        let first = origin(&ledger);
        let expected = SourceTimeoutCaller::project(file!(), expected_line);
        assert_eq!(expected.owner, 8);
        assert_eq!(first.caller, Some(expected));
        assert_eq!(
            (
                first.site,
                first.domain,
                first.phase,
                first.return_disposition
            ),
            (
                SourceTimeoutSite::GenericScope,
                SourceTimeoutDomain::Common,
                SourceTimeoutPhase::ScopeCheck,
                SourceTimeoutReturn::Deadline
            )
        );
        *observed_line = expected_line;
        assert_eq!(owner.check(), Err(SourceServiceError::WorkDeadline));
        assert_eq!(origin(&ledger), first);
        assert!(
            ledger
                .snapshot()
                .timeout
                .later_sticky_work_deadline_observed
        );
        drop(ledger);
        assert!(owner.registry_observation_for_test().1.origin.is_none());
        assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    }
    assert_ne!(observed_lines[0], observed_lines[1]);
    let owner = scope(Duration::from_millis(10), SourceServiceLimits::default());
    expire(owner.work_deadline());
    assert_eq!(owner.check(), Err(SourceServiceError::WorkDeadline));
    let unarmed = owner.registry_observation_for_test().1;
    assert!(!unarmed.armed && unarmed.origin.is_none());
    #[allow(dead_code)]
    struct BeforeOrigin {
        site: SourceTimeoutSite,
        domain: SourceTimeoutDomain,
        phase: SourceTimeoutPhase,
        return_disposition: SourceTimeoutReturn,
        step_add_overflow: bool,
    }
    #[allow(dead_code)]
    struct BeforeSnapshot {
        armed: bool,
        first_error_is_work_deadline: bool,
        origin: Option<BeforeOrigin>,
        later_sticky_work_deadline_observed: bool,
    }
    #[allow(dead_code)]
    struct BeforeState {
        armed: bool,
        origin: Option<BeforeOrigin>,
        sticky: bool,
    }
    eprintln!(
        "WP1090_CALLER_LAYOUT caller={}:{} origin={}:{} snapshot={}:{} state={}:{} before_origin={}:{} before_snapshot={}:{} before_state={}:{}",
        std::mem::size_of::<SourceTimeoutCaller>(),
        std::mem::align_of::<SourceTimeoutCaller>(),
        std::mem::size_of::<SourceTimeoutOrigin>(),
        std::mem::align_of::<SourceTimeoutOrigin>(),
        std::mem::size_of::<SourceTimeoutSnapshot>(),
        std::mem::align_of::<SourceTimeoutSnapshot>(),
        std::mem::size_of::<SourceTimeoutState>(),
        std::mem::align_of::<SourceTimeoutState>(),
        std::mem::size_of::<BeforeOrigin>(),
        std::mem::align_of::<BeforeOrigin>(),
        std::mem::size_of::<BeforeSnapshot>(),
        std::mem::align_of::<BeforeSnapshot>(),
        std::mem::size_of::<BeforeState>(),
        std::mem::align_of::<BeforeState>()
    );
}

#[test]
fn deadline_selector_exact_tie_overflow_and_finite_codes() {
    let start = Instant::now();
    let common = start + Duration::from_secs(1);
    for (step, domain, overflow) in [
        (Duration::from_millis(1), SourceTimeoutDomain::Step, false),
        (Duration::from_secs(1), SourceTimeoutDomain::Tie, false),
        (Duration::from_secs(2), SourceTimeoutDomain::Common, false),
        (Duration::MAX, SourceTimeoutDomain::Common, true),
    ] {
        let selected = SourceTimeoutDeadline::select(common, start, step);
        let observed = selected.origin(
            SourceTimeoutSite::Dns,
            SourceTimeoutPhase::Timer,
            SourceTimeoutReturn::Deadline,
        );
        assert_eq!(observed.domain, domain);
        assert_eq!(observed.step_add_overflow, overflow);
        assert_eq!(
            selected.deadline,
            start.checked_add(step).unwrap_or(common).min(common)
        );
    }
    assert_eq!(SourceTimeoutSite::Headers.code(), 7);
    assert_eq!(SourceTimeoutDomain::Uncomputed.code(), 0);
    assert_eq!(SourceTimeoutPhase::UnattributedPoison.code(), 0);
    assert_eq!(SourceTimeoutReturn::Unknown.code(), 0);
}

#[tokio::test]
async fn actual_timer_common_step_and_later_sticky_preserve_first_creator() {
    for (work, step, domain) in [
        (
            Duration::from_secs(1),
            Duration::from_millis(5),
            SourceTimeoutDomain::Step,
        ),
        (
            Duration::from_millis(50),
            Duration::from_secs(1),
            SourceTimeoutDomain::Common,
        ),
    ] {
        let (owner, ledger) = armed(work);
        let polled = std::cell::Cell::new(false);
        assert_eq!(
            owner
                .during_at(SourceTimeoutSite::Headers, step, async {
                    polled.set(true);
                    std::future::pending::<Result<(), SourceServiceError>>().await
                })
                .await,
            Err(SourceServiceError::WorkDeadline)
        );
        assert!(polled.get());
        let first = origin(&ledger);
        assert!(first.caller.is_none());
        assert_eq!(
            (
                first.site,
                first.domain,
                first.phase,
                first.return_disposition
            ),
            (
                SourceTimeoutSite::Headers,
                domain,
                SourceTimeoutPhase::Timer,
                SourceTimeoutReturn::Deadline
            )
        );
        assert!(
            !ledger
                .snapshot()
                .timeout
                .later_sticky_work_deadline_observed
        );
        let polled = std::cell::Cell::new(false);
        assert_eq!(
            owner
                .during_at(SourceTimeoutSite::Body, step, async {
                    polled.set(true);
                    Ok(())
                })
                .await,
            Err(SourceServiceError::WorkDeadline)
        );
        assert!(!polled.get());
        assert_eq!(origin(&ledger), first);
        assert!(
            ledger
                .snapshot()
                .timeout
                .later_sticky_work_deadline_observed
        );
    }
}

#[tokio::test]
async fn expired_checks_postcheck_primary_and_final_common_are_distinct() {
    for entry in [false, true] {
        let (owner, ledger) = armed(Duration::from_millis(10));
        expire(owner.work_deadline());
        let polled = std::cell::Cell::new(false);
        let result = if entry {
            owner
                .during_at(SourceTimeoutSite::Connect, Duration::from_secs(1), async {
                    polled.set(true);
                    Ok(())
                })
                .await
        } else {
            owner.check()
        };
        assert_eq!(result, Err(SourceServiceError::WorkDeadline));
        assert!(!polled.get());
        let observed = origin(&ledger);
        assert_eq!(observed.caller.is_some(), !entry);
        assert_eq!(observed.domain, SourceTimeoutDomain::Common);
        assert_eq!(
            observed.phase,
            if entry {
                SourceTimeoutPhase::DuringEntryCheck
            } else {
                SourceTimeoutPhase::ScopeCheck
            }
        );
        assert_eq!(
            observed.site,
            if entry {
                SourceTimeoutSite::Connect
            } else {
                SourceTimeoutSite::GenericScope
            }
        );
    }
    for primary in [None, Some(SourceServiceError::UnsupportedSourceOwner)] {
        let (owner, ledger) = armed(Duration::from_secs(1));
        let result = owner
            .during_at(
                SourceTimeoutSite::TlsHandshake,
                Duration::from_millis(1),
                async {
                    // Actually enter the future, then cross its selected step deadline.
                    expire(Instant::now() + Duration::from_millis(3));
                    primary.map_or(Ok(()), Err)
                },
            )
            .await;
        assert_eq!(
            result,
            Err(primary.unwrap_or(SourceServiceError::WorkDeadline))
        );
        let observed = origin(&ledger);
        assert_eq!(
            (observed.domain, observed.phase),
            (SourceTimeoutDomain::Step, SourceTimeoutPhase::Postcheck)
        );
        assert_eq!(
            observed.return_disposition,
            if primary.is_some() {
                SourceTimeoutReturn::CompletedPrimary
            } else {
                SourceTimeoutReturn::Deadline
            }
        );
    }
    let (owner, ledger) = armed(Duration::from_millis(10));
    FINAL_CHECK_DELAY.with(|slot| slot.set(Some(owner.work_deadline())));
    assert_eq!(
        owner
            .during_at(
                SourceTimeoutSite::HttpHandshake,
                Duration::from_secs(1),
                async { Ok(()) }
            )
            .await,
        Err(SourceServiceError::WorkDeadline)
    );
    let observed = origin(&ledger);
    assert_eq!(
        (observed.site, observed.domain, observed.phase),
        (
            SourceTimeoutSite::HttpHandshake,
            SourceTimeoutDomain::Common,
            SourceTimeoutPhase::DuringFinalCheck
        )
    );
}

#[tokio::test]
async fn unarmed_unknown_prior_poison_and_cancellation_keep_original_results() {
    let owner = scope(Duration::from_secs(1), SourceServiceLimits::default());
    assert_eq!(
        owner
            .during(
                Duration::from_millis(1),
                std::future::pending::<Result<(), SourceServiceError>>()
            )
            .await,
        Err(SourceServiceError::WorkDeadline)
    );
    assert!(owner.arm_registry_association_for_test().is_err());
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);

    let (owner, ledger) = armed(Duration::from_secs(1));
    assert_eq!(
        owner.poison(SourceServiceError::WorkDeadline),
        SourceServiceError::WorkDeadline
    );
    assert_eq!(origin(&ledger), SourceTimeoutOrigin::unattributed());
    for first in [
        SourceServiceError::Cancelled,
        SourceServiceError::Limit(SourceCounter::RegistryAttempts),
        SourceServiceError::UnsupportedSourceOwner,
    ] {
        let (owner, ledger) = armed(Duration::from_secs(1));
        assert_eq!(owner.poison(first), first);
        assert_eq!(owner.poison(SourceServiceError::WorkDeadline), first);
        let observed = ledger.snapshot().timeout;
        assert!(!observed.first_error_is_work_deadline && observed.origin.is_none());
        assert_eq!(owner.check(), Err(first));
    }
    let (owner, ledger) = armed(Duration::from_secs(1));
    let waiting = owner.during_at(
        SourceTimeoutSite::Dns,
        Duration::from_secs(1),
        std::future::pending::<Result<(), SourceServiceError>>(),
    );
    let cancel = async {
        tokio::task::yield_now().await;
        owner.cancel();
    };
    let (result, ()) = tokio::join!(waiting, cancel);
    assert_eq!(result, Err(SourceServiceError::Cancelled));
    assert!(ledger.snapshot().timeout.origin.is_none());
}

#[test]
fn joined_competing_creators_install_one_origin_without_scope_lock_reversal() {
    let (owner, ledger) = armed(Duration::from_secs(1));
    let barrier = Arc::new(Barrier::new(3));
    let threads: Vec<_> = [SourceTimeoutSite::Dns, SourceTimeoutSite::Body]
        .into_iter()
        .map(|site| {
            let owner = owner.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                    .unwrap();
                runtime.block_on(owner.during_at(site, Duration::from_millis(1), async {
                    barrier.wait();
                    expire(Instant::now() + Duration::from_millis(3));
                    Ok(())
                }))
            })
        })
        .collect();
    barrier.wait();
    for thread in threads {
        assert_eq!(
            thread.join().unwrap(),
            Err(SourceServiceError::WorkDeadline)
        );
    }
    let observed = origin(&ledger);
    assert!([SourceTimeoutSite::Dns, SourceTimeoutSite::Body].contains(&observed.site));
    assert_eq!(
        (observed.phase, observed.domain),
        (SourceTimeoutPhase::Postcheck, SourceTimeoutDomain::Step)
    );
    assert!(
        ledger
            .snapshot()
            .timeout
            .later_sticky_work_deadline_observed
    );
    let first = observed;
    owner.check().unwrap_err();
    assert_eq!(origin(&ledger), first);
}

#[test]
fn arm_candidates_credit_clone_pin_and_last_drop_disarm_exact_owner() {
    let owner = scope(Duration::from_secs(1), SourceServiceLimits::default());
    // Winner publishes during a real losing candidate's allocation→publication hook.
    let mut winner = None;
    let result = owner.arm_registry_association_with_hook(|| {
        winner = Some(owner.arm_registry_association_for_test().unwrap());
    });
    assert!(matches!(result, Err(SourceServiceError::ForeignScope)));
    let winner = winner.unwrap();
    let expected = (128usize * 8192 * 2)
        + 2 * std::mem::size_of::<RegistryRequestAssociation>()
        + std::mem::size_of::<SourceTimeoutState>()
        + std::mem::size_of::<super::super::buffer_provenance::SourceBufferState>()
        + 64;
    assert_eq!(winner.snapshot().charged_bytes, expected);
    assert_eq!(owner.snapshot().live_buffer_bytes, expected);
    assert!(winner.snapshot().timeout.armed);
    owner.poison(SourceServiceError::WorkDeadline);
    assert_eq!(origin(&winner), SourceTimeoutOrigin::unattributed());
    let clone = winner.clone();
    drop(winner);
    assert!(clone.snapshot().timeout.armed);
    assert_eq!(owner.snapshot().live_buffer_bytes, expected);
    let weak = Arc::downgrade(&owner);
    drop(clone);
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    let cleared = owner.registry_observation_for_test().1;
    assert!(!cleared.armed && cleared.origin.is_none());
    assert!(!cleared.later_sticky_work_deadline_observed);
    assert!(cleared.first_error_is_work_deadline); // Semantic poison was not cleared.
    // Cleared state cannot be observed through a stale strong recorder.
    drop(owner);
    assert!(weak.upgrade().is_none());

    let limits = SourceServiceLimits {
        live_buffer_bytes: expected - 1,
        ..SourceServiceLimits::default()
    };
    let owner = scope(Duration::from_secs(1), limits);
    assert!(matches!(
        owner.arm_registry_association_for_test(),
        Err(SourceServiceError::BufferLimit)
    ));
    assert_eq!(owner.snapshot().live_buffer_bytes, 0);
    assert_eq!(
        owner.first_error_for_test(),
        Some(SourceServiceError::BufferLimit)
    );
}

#[test]
fn concurrent_last_drop_and_timeout_creation_never_leave_uncredited_evidence() {
    let (owner, ledger) = armed(Duration::from_secs(1));
    let start = Arc::new(Barrier::new(2));
    let producer = {
        let owner = owner.clone();
        let start = start.clone();
        std::thread::spawn(move || {
            start.wait();
            owner.poison(SourceServiceError::WorkDeadline)
        })
    };
    start.wait();
    drop(ledger);
    assert_eq!(producer.join().unwrap(), SourceServiceError::WorkDeadline);
    let (counters, cleared, _) = owner.registry_observation_for_test();
    assert_eq!(counters.live_buffer_bytes, 0);
    assert!(!cleared.armed && cleared.origin.is_none());
    assert!(!cleared.later_sticky_work_deadline_observed);
    assert!(cleared.first_error_is_work_deadline);
    let weak = Arc::downgrade(&owner);
    drop(owner);
    assert!(weak.upgrade().is_none());
}
