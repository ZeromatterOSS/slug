use std::collections::hash_map::DefaultHasher;
use std::convert::Infallible;

use super::*;

fn plan(input: &str) -> DottedVersionPlan<'_> {
    DottedVersion::prepare(input, || Ok::<_, Infallible>(())).unwrap()
}
fn version(input: &str) -> DottedVersion {
    plan(input).into_owned()
}
fn language_hash(v: &DottedVersion) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash_normalized(&mut h);
    h.finish()
}
fn native_hash(v: &DottedVersion) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

#[test]
fn pinned_grammar_cutoff_and_exact_invalid_errors() {
    for input in [
        "",
        "experimental.1",
        "1.",
        "1..2",
        ".1",
        "-1",
        "+1",
        " 1",
        "1 ",
        "1_0",
        "1.alpha-",
        "1.α",
        "١",
        "2147483648",
        "1a2147483648",
    ] {
        let error = DottedVersion::prepare(input, || Ok::<_, Infallible>(())).unwrap_err();
        assert!(
            matches!(error, DottedVersionPrepareError::Invalid(_)),
            "{input:?}"
        );
        assert_eq!(
            error.to_string(),
            format!(
                "Dotted version components must all start with the form \\d+([a-z0-9]*?)?(\\d+)? but got '{input}'"
            )
        );
    }
    for input in [
        "0",
        "00000000000000000000000000000000000001",
        "2147483647",
        "1a2147483647",
        "1a2147483648b",
        "1.experimental..☃",
        "1.A_internal.!",
        "1.alpha.",
        "1.internal.\0☃",
        "12ab34cd56",
        "12ab34cd",
    ] {
        assert_eq!(version(input).spelling(), input);
    }
    let v = version("12ab34cd56.00.internal.☃");
    assert_eq!(v.original_component_count(), 2);
    assert_eq!(v.components().len(), 1);
    let c = v.components().next().unwrap();
    assert_eq!(
        (c.first_number(), c.alpha(), c.second_number(), c.spelling()),
        (12, Some("ab34cd"), 56, "12ab34cd56")
    );
    let mut paid = 0;
    DottedVersion::prepare("1.internal..☃", || {
        paid += 1;
        Ok::<_, Infallible>(())
    })
    .unwrap();
    assert_eq!(paid, 2, "ignored tail has no scanned component");
}

#[test]
fn pinned_order_groups_and_normalized_comparison_sign() {
    // DottedVersionTest.testCompareTo's complete ordered groups.
    let groups: &[&[&str]] = &[
        &["0", "0.0.0"],
        &["0.1", "0.01"],
        &["0.2", "0.2.0"],
        &["0.2.1"],
        &["1.2alpha"],
        &["1.2alpha1"],
        &["1.2alpha2"],
        &["1.2beta1"],
        &["1.2beta12"],
        &["1.2beta12.1"],
        &["1.2.0", "1.2"],
        &["1.20"],
        &["10", "10.0"],
        &["10.0.0.10A255"],
        &["10.2"],
        &["10.2.0.10P99q"],
        &["10.10.simulator.internal", "10.10"],
    ];
    for (i, a) in groups.iter().enumerate() {
        for (j, b) in groups.iter().enumerate() {
            for x in *a {
                for y in *b {
                    assert_eq!(version(x).normalized_cmp(&version(y)), i.cmp(&j), "{x}/{y}");
                }
            }
        }
    }
    for (a, b) in [("1A", "1a"), ("1ab", "1abcd"), ("1a9", "1a10"), ("1a", "1")] {
        let order = version(a).normalized_cmp(&version(b));
        let exposed = match order {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        };
        assert_eq!(
            exposed, -1,
            "ComparisonChain exposes sign, not String.compareTo magnitude"
        );
    }
}

#[test]
fn normalized_hash_and_spelling_sensitive_native_state_are_distinct() {
    for group in [
        &["1", "1.0", "01.00", "1.internal.☃"][..],
        &["1.2x", "1.2x0", "1.2x0.0"][..],
        &["10.2.0.10P99q", "10.2.0.10P99q0"][..],
    ] {
        let first = version(group[0]);
        for spelling in &group[1..] {
            let other = version(spelling);
            assert_eq!(first.normalized_cmp(&other), Ordering::Equal);
            assert_eq!(language_hash(&first), language_hash(&other));
            assert_ne!(first, other);
            assert_ne!(native_hash(&first), native_hash(&other));
            assert_eq!(other, version(spelling));
        }
    }
    let a = version("01.00.0.internal");
    assert_eq!(a.original_component_count(), 3);
    assert_eq!(a.components().next().unwrap().spelling(), "01");
    assert_eq!(a.components().len(), 1);
    assert_ne!(language_hash(&version("1A")), language_hash(&version("1a")));
    let weak = Arc::downgrade(&a.0);
    let shared = a.dupe();
    assert!(Arc::ptr_eq(&a.0, &shared.0));
    drop(a);
    assert!(weak.upgrade().is_some());
    drop(shared);
    assert!(weak.upgrade().is_none());
}

#[test]
fn complete_storage_and_transfer_forecasts_are_checked() {
    let input = "01.2a3.00.internal.abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz";
    let p = plan(input);
    assert_eq!(p.original_component_count(), 3);
    assert_eq!(p.normalized_component_count(), 2);
    assert_eq!(p.construction_records(), 4 + 2 + 1);
    assert_eq!(p.spelling().as_ptr(), input.as_ptr(), "plan only borrows");
    let bound = p.storage();
    let v = p.into_owned();
    assert_eq!(v.storage(), bound);
    let actual = size_of::<DottedVersion>()
        + size_of::<Data>()
        + 2 * size_of::<usize>()
        + v.0.spelling.capacity()
        + std::mem::size_of_val(v.0.components.as_ref());
    assert!(bound.retained_bytes() >= actual);
    assert!(
        bound.construction_peak_bytes()
            >= bound.retained_bytes()
                + size_of::<Vec<Component>>()
                + std::mem::size_of_val(v.0.components.as_ref())
    );
    let longer = version(&(input.to_owned() + "☃"));
    assert_eq!(
        longer.storage().retained_bytes() - v.storage().retained_bytes(),
        "☃".len()
    );
    assert!(storage_bound(usize::MAX, 1).is_none());
    assert!(storage_bound(1, usize::MAX).is_none());
    assert!(
        storage_bound(1, 0).is_some(),
        "forecast arithmetic does not invent a semantic constructor"
    );
}

#[test]
fn admission_precedes_scanning_and_owned_construction() {
    let error = DottedVersion::prepare("☃", || Err("work exhausted")).unwrap_err();
    assert_eq!(
        error,
        DottedVersionPrepareError::Admission("work exhausted")
    );
    let mut left = 1usize;
    let error = DottedVersion::prepare("1.☃", || {
        left = left.checked_sub(1).ok_or("work exhausted")?;
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        error,
        DottedVersionPrepareError::Admission("work exhausted")
    );
    assert_eq!(left, 0);
    let input = "1.2a3.0.internal.☃";
    let p = plan(input);
    let work = p.construction_records();
    let bytes = p.storage().construction_peak_bytes();
    fn admit(
        p: DottedVersionPlan<'_>,
        work_limit: usize,
        byte_limit: usize,
    ) -> Result<DottedVersion, DottedVersionPlan<'_>> {
        if p.construction_records() > work_limit
            || p.storage().construction_peak_bytes() > byte_limit
        {
            Err(p)
        } else {
            Ok(p.into_owned())
        }
    }
    let p = admit(p, work - 1, bytes).unwrap_err();
    assert_eq!(p.spelling().as_ptr(), input.as_ptr());
    let p = admit(p, work, bytes - 1).unwrap_err();
    assert_eq!(p.spelling().as_ptr(), input.as_ptr());
    assert_eq!(admit(p, work, bytes).unwrap(), version(input));
}
