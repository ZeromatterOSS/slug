use super::*;

#[test]
fn contextual_array_scratch_and_identity_preserve_shared_requests_and_stopping() {
    for count in [0, 1, 17, usize::MAX] {
        let mut a = Budget::default();
        let mut b = a;
        assert_eq!(
            a.array::<u64>(count).is_ok(),
            b.array_with_context::<u64>(CONTEXT, count).is_ok()
        );
        assert_eq!(state(a), state(b));
        let mut a = Budget {
            bytes: 1,
            ..Budget::default()
        };
        let mut b = a;
        assert_eq!(
            a.scratch(count, 19).is_ok(),
            b.scratch_with_context(CONTEXT, count, 19).is_ok()
        );
        assert_eq!(state(a), state(b));
    }
    let mut saturated = Budget::default();
    let error = saturated
        .array_with_context::<usize>(CONTEXT, usize::MAX)
        .unwrap_err();
    assert!(error.contains("kind=bytes-cap"));
    assert!(error.contains(&format!(
        "requested_bytes={} record_total=0 byte_total={}",
        usize::MAX,
        usize::MAX
    )));
    let identity = BzlModuleIdentity {
        label: CanonicalLabel::parse("@@//pkg:module.bzl").unwrap(),
        source: slug_build_api_v2::ModuleSourceOrigin::Workspace("/identity/module.bzl".into()),
        repository_mapping: Arc::from([
            (
                slug_identity_v2::ApparentRepoName::new("a").unwrap(),
                slug_identity_v2::CanonicalRepoName::new("a+").unwrap(),
            ),
            (
                slug_identity_v2::ApparentRepoName::new("long_alias").unwrap(),
                slug_identity_v2::CanonicalRepoName::new("long_canonical+").unwrap(),
            ),
        ]),
    };
    for records in [0, RECORD_LIMIT - 2, RECORD_LIMIT, usize::MAX] {
        let mut a = Budget {
            records,
            ..Budget::default()
        };
        let mut b = a;
        assert_eq!(
            a.identity(&identity).is_ok(),
            b.identity_with_context(CONTEXT, &identity).is_ok()
        );
        assert_eq!(state(a), state(b));
    }
    let mut calls = 0;
    let result = identity_charges(&identity, |_, _| {
        calls += 1;
        if calls == 2 {
            Err("stop".into())
        } else {
            Ok(())
        }
    });
    assert_eq!(result.unwrap_err(), "stop");
    assert_eq!(calls, 2);
}

#[test]
fn contextual_peak_overflow_distinguishes_unreached_extra_without_mutation() {
    let logical = Budget {
        records: 2,
        bytes: usize::MAX,
        depth: 7,
    };
    let scratch = Budget {
        records: 3,
        bytes: 1,
        depth: 11,
    };
    let error = admit_peak(CONTEXT, logical, scratch, usize::MAX).unwrap_err();
    assert!(error.contains("operation=joint-peak kind=logical-scratch-overflow"));
    assert!(error.contains("base_total=overflow extra_total=not-reached"));
    let scratch = Budget {
        bytes: 0,
        ..scratch
    };
    let error = admit_peak(CONTEXT, logical, scratch, 1).unwrap_err();
    assert!(error.contains("kind=extra-overflow"));
    assert!(error.contains(&format!("base_total={} extra_total=overflow", usize::MAX)));
    assert_eq!(state(logical), (2, usize::MAX, 7));
    assert_eq!(state(scratch), (3, 0, 11));
    let error = limit_with_context(CONTEXT);
    assert_eq!(
        error,
        format!(
            "{} (operation=checked-arithmetic kind=checked-arithmetic owner=semantic-inputs phase=input-validation first_ledger=validation)",
            limit()
        )
    );
    assert!(!error.contains("before_"));
}

#[test]
fn contextual_index_failure_reports_local_proposed_state_without_committing_scratch() {
    let mut scratch = Budget {
        records: 7,
        bytes: 11,
        depth: 13,
    };
    let before = scratch;
    let error =
        admit_index_growth::<usize>(CONTEXT, Budget::default(), &mut scratch, usize::MAX, 0)
            .unwrap_err();
    assert_eq!(state(scratch), state(before));
    assert!(error.contains("operation=charge kind=bytes-overflow"));
    assert!(error.contains("owner=semantic-inputs phase=input-validation first_ledger=work"));
    assert!(error.contains("before_records=7 before_bytes=11 before_depth=13"));
    assert!(error.contains("after_records=8 after_bytes=11 after_depth=13"));
}

#[test]
fn contextual_maximum_formatting_and_closed_context_replacements_are_bounded() {
    for owner in PeakOwner::ALL {
        for phase in PeakPhase::ALL {
            for ledger in FirstLedger::ALL {
                let context = PeakContext::new(*owner, PeakPhase::InputDepth, FirstLedger::Logical)
                    .with_phase(*phase)
                    .with_ledger(*ledger);
                let before = Budget {
                    records: usize::MAX,
                    bytes: usize::MAX,
                    depth: usize::MAX,
                };
                let mut budget = before;
                let charge = budget
                    .charge_with_context(context, usize::MAX, usize::MAX)
                    .unwrap_err();
                let mut budget = Budget {
                    records: 0,
                    ..before
                };
                let byte_overflow = budget
                    .charge_with_context(context, usize::MAX, usize::MAX)
                    .unwrap_err();
                let depth = budget.depth_with_context(context, usize::MAX).unwrap_err();
                let peak = admit_peak(context, before, before, usize::MAX).unwrap_err();
                let arithmetic = limit_with_context(context);
                for error in [charge, byte_overflow, depth, peak, arithmetic] {
                    assert!(error.len() <= 1024, "{}", error.len());
                    assert!(error.contains(context.owner()));
                    assert!(error.contains(context.phase()));
                    assert!(error.contains(context.first_ledger()));
                }
            }
        }
    }
}
