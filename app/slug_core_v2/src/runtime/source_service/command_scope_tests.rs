use super::*;
use crate::runtime::repository_physical_root::RepositoryRootResourceLimits;

const BANK_BYTES: usize = 1024 * 1024;
fn bank() -> Arc<RepositoryRootResources> {
    RepositoryRootResources::new(RepositoryRootResourceLimits::new(BANK_BYTES, 16, 1).unwrap())
        .unwrap()
}
fn workspace() -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new("/private-workspace-token").unwrap()
}
fn fresh(
    resources: &Arc<RepositoryRootResources>,
) -> Result<Arc<SourceServiceScope>, SourceServiceError> {
    let work = Instant::now() + Duration::from_secs(5);
    SourceServiceScope::new_command(
        workspace(),
        work,
        work + Duration::from_secs(1),
        SourceServiceLimits::default(),
        resources.clone(),
    )
}

#[test]
fn command_scope_validates_finite_clocks_before_shared_bank_admission() {
    let resources = bank();
    let baseline = resources.snapshot();
    let now = Instant::now();
    for (work, cleanup) in [
        (now, now + Duration::from_secs(1)),
        (now + Duration::from_secs(28), now + Duration::from_secs(29)),
        (now + Duration::from_secs(5), now + Duration::from_secs(4)),
        (now + Duration::from_secs(5), now + Duration::from_secs(9)),
    ] {
        assert!(matches!(
            SourceServiceScope::new_command(
                workspace(),
                work,
                cleanup,
                SourceServiceLimits::default(),
                resources.clone()
            ),
            Err(SourceServiceError::InvalidLimits)
        ));
    }
    let limits = SourceServiceLimits {
        live_buffer_bytes: 0,
        ..SourceServiceLimits::default()
    };
    assert!(matches!(
        SourceServiceScope::new_command(
            workspace(),
            now + Duration::from_secs(5),
            now + Duration::from_secs(6),
            limits,
            resources.clone()
        ),
        Err(SourceServiceError::InvalidLimits)
    ));
    assert_eq!(resources.snapshot().retained_bytes, baseline.retained_bytes);
    assert_eq!(
        resources.snapshot().peak_retained_bytes,
        baseline.peak_retained_bytes
    );
    let valid = fresh(&resources).unwrap();
    assert_eq!(valid.limits(), SourceServiceLimits::default());
    assert!(valid.check().is_ok());
    assert!(!format!("{valid:?}").contains("private-workspace-token"));
}

#[test]
fn command_scope_control_follows_last_arc_and_default_constructor_stays_unbanked() {
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let scope = fresh(&resources).unwrap();
    let retained = resources.snapshot().retained_bytes;
    assert!(retained > baseline + std::mem::size_of::<SourceServiceScope>());
    assert!(Arc::ptr_eq(scope.command_resources().unwrap(), &resources));
    let weak = Arc::downgrade(&scope);
    let clone = scope.clone();
    drop(scope);
    assert!(weak.upgrade().is_some());
    assert_eq!(resources.snapshot().retained_bytes, retained);
    drop(clone);
    assert!(weak.upgrade().is_none());
    drop(weak);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
    let work = Instant::now() + Duration::from_secs(5);
    let diagnostic = SourceServiceScope::new_for_test(
        workspace(),
        work,
        work + Duration::from_secs(1),
        SourceServiceLimits::default(),
    )
    .unwrap();
    assert!(diagnostic.command_resources().is_none());
    let buffer = diagnostic.reserve_buffer(17).unwrap();
    let decoder = diagnostic.reserve_decoder(31).unwrap();
    assert_eq!(resources.snapshot().retained_bytes, baseline);
    drop((buffer, decoder, diagnostic));
}

#[test]
fn command_scopes_have_independent_counters_and_cancellation_on_one_shared_bank() {
    let resources = bank();
    let a = fresh(&resources).unwrap();
    let b = fresh(&resources).unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert!(Arc::ptr_eq(
        a.command_resources().unwrap(),
        b.command_resources().unwrap()
    ));
    a.charge(SourceCounter::SourceAttempts, 1).unwrap();
    assert_eq!(a.snapshot().source_attempts, 1);
    assert_eq!(b.snapshot().source_attempts, 0);
    let buffer = a.reserve_buffer(128).unwrap();
    let decoder = a.reserve_decoder(256).unwrap();
    assert_eq!(a.snapshot().live_buffer_bytes, 128);
    assert_eq!(a.snapshot().live_decoder_bytes, 256);
    assert_eq!(b.snapshot().live_buffer_bytes, 0);
    a.cancel();
    assert_eq!(a.check(), Err(SourceServiceError::Cancelled));
    assert!(b.check().is_ok());
    drop((buffer, decoder));
    assert_eq!(a.snapshot().live_buffer_bytes, 0);
    assert_eq!(a.snapshot().live_decoder_bytes, 0);
    assert_eq!(a.snapshot().source_attempts, 1);
    assert_eq!(a.check(), Err(SourceServiceError::Cancelled));
}

#[test]
fn command_buffer_and_decoder_credits_outlive_command_arc_and_constrain_next_factory() {
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let a = fresh(&resources).unwrap();
    let control = command_control_bytes(&workspace()).unwrap();
    let buffer = a.reserve_buffer(64).unwrap();
    let decoder = a.reserve_decoder(4096).unwrap();
    assert!(control > 64 + std::mem::size_of::<SourceBufferReservation>());
    let weak = Arc::downgrade(&a);
    drop(a);
    assert!(weak.upgrade().is_some());
    // An unrelated retained native owner occupies the rest of the SAME bank.
    let other = resources
        .reserve(BANK_BYTES - resources.snapshot().retained_bytes, 0, 0)
        .unwrap();
    assert!(matches!(
        fresh(&resources),
        Err(SourceServiceError::Allocation)
    ));
    drop(buffer);
    assert!(weak.upgrade().is_some());
    assert!(matches!(
        fresh(&resources),
        Err(SourceServiceError::Allocation)
    ));
    drop(decoder);
    assert!(weak.upgrade().is_none());
    drop(weak);
    let b = fresh(&resources).unwrap();
    assert_eq!(b.snapshot().source_attempts, 0);
    assert!(b.reserve_buffer(64).is_ok());
    drop(b);
    drop(other);
    assert_eq!(resources.snapshot().retained_bytes, baseline);
}

#[test]
fn shared_bank_refusal_is_sticky_and_does_not_increment_scope_counters() {
    for decoder in [false, true] {
        let resources = bank();
        let baseline = resources.snapshot().retained_bytes;
        let a = fresh(&resources).unwrap();
        let b = fresh(&resources).unwrap();
        let held = a.reserve_buffer(1024).unwrap();
        let full = resources
            .reserve(BANK_BYTES - resources.snapshot().retained_bytes, 0, 0)
            .unwrap();
        let before = resources.snapshot().retained_bytes;
        let refused = if decoder {
            b.reserve_decoder(1).map(drop)
        } else {
            b.reserve_buffer(1).map(drop)
        };
        assert_eq!(refused, Err(SourceServiceError::Allocation));
        assert_eq!(resources.snapshot().retained_bytes, before);
        assert_eq!(b.snapshot().live_buffer_bytes, 0);
        assert_eq!(b.snapshot().peak_buffer_bytes, 0);
        assert_eq!(b.snapshot().live_decoder_bytes, 0);
        assert_eq!(b.snapshot().peak_decoder_bytes, 0);
        drop(full);
        drop(held);
        assert_eq!(b.check(), Err(SourceServiceError::Allocation));
        assert_eq!(
            b.reserve_buffer(0).err(),
            Some(SourceServiceError::Allocation)
        );
        drop((a, b));
        assert_eq!(resources.snapshot().retained_bytes, baseline);
    }
}
