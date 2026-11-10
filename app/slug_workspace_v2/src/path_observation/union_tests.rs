//! Exact union/reference and charged-owner checks, without filesystem IO.
use std::sync::Arc;

use dupe::Dupe;

use crate::ImageChargedPath;
use crate::ImageDescriptorBudget;
use crate::ImageObservationData;
use crate::ImageObservationOperation;
use crate::ImageObservationRequest;
use crate::ImageObservationResult;
use crate::path_observation::*;

fn demand(
    namespace: PathObservationNamespace,
    operation: PathObservationOperation,
    name: &str,
) -> PathObservationDemand {
    PathObservationDemand::new(
        namespace,
        NormalizedAbsolutePath::new(name).unwrap(),
        operation,
    )
}
fn missing() -> Arc<PathObservationResult> {
    Arc::new(PathObservationResult::Lstat(PathOperationResult::Missing))
}
fn reference(
    left: &PathObservationEpoch,
    right: &PathObservationEpoch,
) -> Result<PathObservationEpoch, PathObservationEpochError> {
    PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .chain(right.observations())
            .map(|(d, r)| (d.dupe(), r.dupe())),
    )
}

#[test]
fn union_matches_shared_reference_and_reuses_noop_owners() {
    let a = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/union/a",
    );
    let z = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/union/z",
    );
    let first = missing();
    let foreign = missing();
    assert!(!Arc::ptr_eq(&first, &foreign));
    let left =
        PathObservationEpoch::from_shared([(a.dupe(), first.dupe()), (z.dupe(), first.dupe())])
            .unwrap();
    let equal =
        PathObservationEpoch::from_shared([(z.dupe(), foreign.dupe()), (a.dupe(), foreign.dupe())])
            .unwrap();
    let subset = PathObservationEpoch::from_shared([(a.dupe(), foreign.dupe())]).unwrap();
    let empty = PathObservationEpoch::empty();
    for right in [&empty, &left, &equal, &subset] {
        let expected = reference(&left, right).unwrap();
        let counts = (Arc::strong_count(&first), Arc::strong_count(&foreign));
        let actual = left.try_union(right).unwrap();
        assert_eq!(actual, expected);
        assert!(std::ptr::eq(actual.observations(), left.observations()));
        assert!(Arc::ptr_eq(actual.get(&a).unwrap(), &first));
        assert_eq!(
            (Arc::strong_count(&first), Arc::strong_count(&foreign)),
            counts
        );
    }
    let actual = empty.try_union(&subset).unwrap();
    assert_eq!(actual, reference(&empty, &subset).unwrap());
    assert!(std::ptr::eq(actual.observations(), subset.observations()));
    assert!(std::ptr::eq(
        empty.try_union(&empty).unwrap().observations(),
        empty.observations()
    ));

    let file = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::FileBytes,
        "/union/a",
    );
    let materialized = |id| {
        demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(id)),
            PathObservationOperation::Lstat,
            "/union/a",
        )
    };
    let additions = PathObservationEpoch::from_shared([
        (materialized(2), first.dupe()),
        (
            file.dupe(),
            Arc::new(PathObservationResult::FileBytes(
                PathOperationResult::Present(Arc::from(&b"bytes"[..])),
            )),
        ),
        (materialized(1), first.dupe()),
        (a.dupe(), foreign.dupe()),
    ])
    .unwrap();
    for (left, right) in [
        (&left, &additions),
        (&additions, &left),
        (&subset, &additions),
        (&additions, &equal),
    ] {
        let actual = left.try_union(right).unwrap();
        assert_eq!(actual, reference(left, right).unwrap());
        for (key, result) in left.observations() {
            assert!(Arc::ptr_eq(actual.get(key).unwrap(), result));
        }
        let mut keys = left
            .observations()
            .keys()
            .chain(right.observations().keys())
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();
        keys.dedup();
        assert_eq!(
            actual.observations().keys().cloned().collect::<Vec<_>>(),
            keys
        );
    }
}

#[test]
fn union_conflicts_preserve_first_sorted_reference_error() {
    let keys = [
        demand(
            PathObservationNamespace::Host,
            PathObservationOperation::Lstat,
            "/union/z",
        ),
        demand(
            PathObservationNamespace::Host,
            PathObservationOperation::FileBytes,
            "/union/a",
        ),
        demand(
            PathObservationNamespace::Host,
            PathObservationOperation::Lstat,
            "/union/a",
        ),
        demand(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
            PathObservationOperation::Lstat,
            "/union/a",
        ),
    ];
    let value = |key: &PathObservationDemand, changed| {
        Arc::new(match key.operation() {
            PathObservationOperation::FileBytes => PathObservationResult::FileBytes(
                PathOperationResult::Present(Arc::from(if changed {
                    &b"changed"[..]
                } else {
                    &b"original"[..]
                })),
            ),
            _ => PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                PathNodeKind::RegularFile,
                1,
                2,
                3,
                4,
                if changed { 0o600 } else { 0o644 },
            ))),
        })
    };
    let left =
        PathObservationEpoch::from_shared(keys.iter().map(|key| (key.dupe(), value(key, false))))
            .unwrap();
    for changed in [vec![0, 1, 2, 3], vec![0, 1], vec![0], vec![3]] {
        let right = PathObservationEpoch::from_shared(
            keys.iter()
                .enumerate()
                .rev()
                .map(|(index, key)| (key.dupe(), value(key, changed.contains(&index)))),
        )
        .unwrap();
        let first = changed.iter().map(|index| &keys[*index]).min().unwrap();
        let expected = reference(&left, &right).unwrap_err();
        assert_eq!(
            expected,
            PathObservationEpochError::ConflictingDemand(first.dupe())
        );
        assert_eq!(left.try_union(&right).unwrap_err(), expected);
    }
}

fn image_row(bank: &ImageDescriptorBudget) -> (PathObservationDemand, Arc<PathObservationResult>) {
    let pin = |text| {
        ImageChargedPath::pin_existing(NormalizedAbsolutePath::new(text).unwrap(), bank).unwrap()
    };
    let issued = pin("/union/image");
    let request = ImageObservationRequest::new(pin("/union"), ImageObservationOperation::Lstat);
    let result = Arc::new(PathObservationResult::InputImage(
        ImageObservationResult::new(
            issued.dupe(),
            request.dupe(),
            ImageObservationData::Lstat(PathOperationResult::Missing),
            bank.try_reserve(64).unwrap(),
        )
        .unwrap(),
    ));
    (
        PathObservationDemand::input_image(PathObservationNamespace::Host, issued, request)
            .unwrap(),
        result,
    )
}
fn assert_bank(
    key: &PathObservationDemand,
    result: &Arc<PathObservationResult>,
    own: &ImageDescriptorBudget,
    foreign: &ImageDescriptorBudget,
) {
    assert!(key.image_path().unwrap().same_budget(own));
    assert!(!key.image_path().unwrap().same_budget(foreign));
    let PathObservationOperation::InputImage(request) = key.operation() else {
        panic!("image key")
    };
    assert!(request.same_budget(own));
    assert!(!request.same_budget(foreign));
    let PathObservationResult::InputImage(image) = result.as_ref() else {
        panic!("image result")
    };
    assert!(image.same_budget(own));
    assert!(!image.same_budget(foreign));
}

#[test]
fn union_growth_preserves_left_charged_key_and_result_owners() {
    let left_bank = ImageDescriptorBudget::new(4096);
    let right_bank = ImageDescriptorBudget::new(4096);
    assert!(!left_bank.same_instance(&right_bank));
    let (left_key, left_result) = image_row(&left_bank);
    let (right_key, right_result) = image_row(&right_bank);
    assert_eq!(left_key, right_key);
    assert_eq!(left_result, right_result);
    assert!(!Arc::ptr_eq(&left_result, &right_result));
    assert_bank(&left_key, &left_result, &left_bank, &right_bank);
    assert_bank(&right_key, &right_result, &right_bank, &left_bank);
    let left_weak = Arc::downgrade(&left_result);
    let right_weak = Arc::downgrade(&right_result);
    let left = PathObservationEpoch::from_shared([(left_key.dupe(), left_result.dupe())]).unwrap();
    let right =
        PathObservationEpoch::from_shared([(right_key.dupe(), right_result.dupe())]).unwrap();
    let used = (left_bank.used_bytes(), right_bank.used_bytes());
    let noop = left.try_union(&right).unwrap();
    assert_eq!(noop, reference(&left, &right).unwrap());
    assert!(std::ptr::eq(noop.observations(), left.observations()));
    for (key, result) in noop.observations() {
        assert_bank(key, result, &left_bank, &right_bank);
    }
    drop(noop);
    let added_key = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/union/added",
    );
    let right_growth = PathObservationEpoch::from_shared([
        (added_key.dupe(), missing()),
        (right_key.dupe(), right_result.dupe()),
    ])
    .unwrap();
    let grown = left.try_union(&right_growth).unwrap();
    assert_eq!(grown, reference(&left, &right_growth).unwrap());
    assert_eq!(grown.observations().len(), 2);
    assert!(!std::ptr::eq(grown.observations(), left.observations()));
    for (key, result) in grown.observations() {
        if key == &left_key {
            assert_bank(key, result, &left_bank, &right_bank);
            assert!(Arc::ptr_eq(result, &left_result));
        }
    }
    assert_eq!((left_bank.used_bytes(), right_bank.used_bytes()), used);
    drop((
        left,
        right,
        right_growth,
        left_key,
        right_key,
        left_result,
        right_result,
    ));
    assert!(right_weak.upgrade().is_none());
    assert_eq!(right_bank.used_bytes(), 0);
    assert!(left_weak.upgrade().is_some());
    assert_eq!(left_bank.used_bytes(), used.0);
    drop(grown);
    assert!(left_weak.upgrade().is_none());
    assert_eq!(left_bank.used_bytes(), 0);
    assert_eq!(right_bank.used_bytes(), 0);
}

fn key_hash(key: &PathObservationDemand) -> u64 {
    use std::hash::Hash;
    use std::hash::Hasher;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hash);
    hash.finish()
}

#[test]
fn union_proper_superset_reuses_right_map_only_after_full_left_custody() {
    let a = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/superset/a",
    );
    let z = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/superset/z",
    );
    let a_result = missing();
    let z_result = missing();
    let left = PathObservationEpoch::from_shared([
        (a.dupe(), a_result.dupe()),
        (z.dupe(), z_result.dupe()),
    ])
    .unwrap();
    let extra = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/superset/m",
    );
    let right = PathObservationEpoch::from_shared(
        left.observations()
            .iter()
            .map(|(d, r)| (d.dupe(), r.dupe()))
            .chain([(extra.dupe(), missing())]),
    )
    .unwrap();
    let actual = left.try_union(&right).unwrap();
    let expected = reference(&left, &right).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.observations().len(), 3);
    assert_eq!(
        actual.observations().keys().cloned().collect::<Vec<_>>(),
        vec![a.dupe(), extra, z.dupe()]
    );
    for (key, result) in left.observations() {
        let (retained_key, retained_result) = actual.observations().get_key_value(key).unwrap();
        assert!(retained_key.same_retention_owners(key));
        assert!(Arc::ptr_eq(retained_result, result));
        assert_eq!(key_hash(retained_key), key_hash(key));
        let (right_key, right_result) = right.observations().get_key_value(key).unwrap();
        assert!(right_key.same_retention_owners(key));
        assert!(Arc::ptr_eq(right_result, result));
    }
    // Baseline causal RED: all facts and complete LEFT custody above already
    // pass. Only reconstructing a map that RIGHT already owns fails here.
    assert!(std::ptr::eq(actual.observations(), right.observations()));
}

fn assert_incompatible_superset_falls_back(
    left_key: PathObservationDemand,
    right_key: PathObservationDemand,
    left_result: Arc<PathObservationResult>,
    right_result: Arc<PathObservationResult>,
    key_owners_match: bool,
) {
    assert_eq!(left_key, right_key);
    assert_eq!(left_result, right_result);
    assert_eq!(left_key.same_retention_owners(&right_key), key_owners_match);
    let left = PathObservationEpoch::from_shared([(left_key.dupe(), left_result.dupe())]).unwrap();
    let addition = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/superset/new",
    );
    let right =
        PathObservationEpoch::from_shared([(right_key, right_result), (addition, missing())])
            .unwrap();
    let actual = left.try_union(&right).unwrap();
    assert_eq!(actual, reference(&left, &right).unwrap());
    assert_eq!(actual.observations().len(), 2);
    let (retained_key, retained_result) = actual.observations().get_key_value(&left_key).unwrap();
    assert!(retained_key.same_retention_owners(&left_key));
    assert!(Arc::ptr_eq(retained_result, &left_result));
    assert!(!std::ptr::eq(actual.observations(), right.observations()));
    assert!(!std::ptr::eq(actual.observations(), left.observations()));
}

#[test]
fn union_superset_isolates_path_windows_and_result_owner_mismatches() {
    let key = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/superset/a",
    );
    let mut foreign_path = key.dupe();
    foreign_path.path = NormalizedAbsolutePath::new(key.path().as_path().to_path_buf()).unwrap();
    let result = missing();
    assert_incompatible_superset_falls_back(
        key.dupe(),
        foreign_path,
        result.dupe(),
        result.dupe(),
        false,
    );
    let foreign_result = Arc::new(result.as_ref().clone());
    assert!(!Arc::ptr_eq(&result, &foreign_result));
    assert_incompatible_superset_falls_back(key.dupe(), key, result, foreign_result, true);

    for constructor in [
        PathObservationDemand::windows_long_path,
        PathObservationDemand::windows_option_path_long_name,
    ] {
        let key = constructor(
            NormalizedAbsolutePath::new("/superset/windows").unwrap(),
            Arc::from(&[67u16, 58, 92, 97][..]),
        );
        let mut foreign_windows = key.dupe();
        foreign_windows.windows_long_path_input = Some(Arc::from(
            key.windows_long_path_input.as_ref().unwrap().as_ref(),
        ));
        let result = match key.operation() {
            PathObservationOperation::WindowsLongPath => Arc::new(
                PathObservationResult::WindowsLongPath(Arc::from(&[67u16, 58, 92, 97][..])),
            ),
            PathObservationOperation::WindowsOptionPathLongName => {
                // Keep a real constructor-produced closed operation/result pair.
                Arc::new(PathObservationResult::WindowsOptionPathLongName(
                    WindowsOptionPathLongNameOutcome::IOExceptionFallback,
                ))
            }
            _ => unreachable!(),
        };
        assert_incompatible_superset_falls_back(key, foreign_windows, result.dupe(), result, false);
    }
}

#[test]
fn union_superset_keeps_same_bank_issued_and_anchor_credits_independently() {
    let bank = ImageDescriptorBudget::new(16 * 1024);
    let anchor =
        ImageChargedPath::pin_existing(NormalizedAbsolutePath::new("/superset").unwrap(), &bank)
            .unwrap();
    let issued = ImageChargedPath::pin_existing(
        NormalizedAbsolutePath::new("/superset/image").unwrap(),
        &bank,
    )
    .unwrap();
    let request = ImageObservationRequest::new(anchor.dupe(), ImageObservationOperation::Lstat);
    let result = Arc::new(PathObservationResult::InputImage(
        ImageObservationResult::new(
            issued.dupe(),
            request.dupe(),
            ImageObservationData::Lstat(PathOperationResult::Missing),
            bank.try_reserve(64).unwrap(),
        )
        .unwrap(),
    ));
    let left = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        issued.dupe(),
        request.dupe(),
    )
    .unwrap();
    let changed_issued = issued.retain_charge(bank.try_reserve(1).unwrap()).unwrap();
    assert!(changed_issued.same_budget(&bank));
    assert!(Arc::ptr_eq(
        &changed_issued.normalized_ref().path,
        &issued.normalized_ref().path
    ));
    let right = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        changed_issued,
        request.dupe(),
    )
    .unwrap();
    // Exact SAME outer result and request owner: only issued charged block differs.
    assert_incompatible_superset_falls_back(
        left.dupe(),
        right,
        result.dupe(),
        result.dupe(),
        false,
    );

    let changed_anchor = anchor.retain_charge(bank.try_reserve(1).unwrap()).unwrap();
    assert!(changed_anchor.same_budget(&bank));
    assert!(Arc::ptr_eq(
        &changed_anchor.normalized_ref().path,
        &anchor.normalized_ref().path
    ));
    let changed_request =
        ImageObservationRequest::new(changed_anchor, ImageObservationOperation::Lstat);
    let right = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        issued.dupe(),
        changed_request,
    )
    .unwrap();
    // Exact SAME outer result and issued charged block: only request anchor differs.
    assert_incompatible_superset_falls_back(left, right, result.dupe(), result.dupe(), false);
    drop((result, request, issued, anchor));
    assert_eq!(bank.used_bytes(), 0);
}

#[test]
fn union_superset_checks_late_conflict_after_incompatible_equal_prefix() {
    let a = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/superset/a",
    );
    let z = demand(
        PathObservationNamespace::Host,
        PathObservationOperation::Lstat,
        "/superset/z",
    );
    let left =
        PathObservationEpoch::from_shared([(a.dupe(), missing()), (z.dupe(), missing())]).unwrap();
    let changed = || {
        Arc::new(PathObservationResult::Lstat(PathOperationResult::Present(
            PathLstat::new(PathNodeKind::Directory, 1, 2, 3, 4, 0o755),
        )))
    };
    for two_conflicts in [false, true] {
        let right = PathObservationEpoch::from_shared([
            (a.dupe(), if two_conflicts { changed() } else { missing() }),
            (
                demand(
                    PathObservationNamespace::Host,
                    PathObservationOperation::Lstat,
                    "/superset/m",
                ),
                missing(),
            ),
            (z.dupe(), changed()),
        ])
        .unwrap();
        let expected = reference(&left, &right).unwrap_err();
        assert_eq!(
            expected,
            PathObservationEpochError::ConflictingDemand(if two_conflicts {
                a.dupe()
            } else {
                z.dupe()
            })
        );
        assert_eq!(left.try_union(&right).unwrap_err(), expected);
        assert_eq!(left.observations().len(), 2);
        assert!(matches!(
            left.get(&a).unwrap().as_ref(),
            PathObservationResult::Lstat(PathOperationResult::Missing)
        ));
    }
}

#[test]
fn union_larger_compatible_right_missing_left_key_uses_full_growth() {
    let key = |name| {
        demand(
            PathObservationNamespace::Host,
            PathObservationOperation::Lstat,
            name,
        )
    };
    let a = key("/count-gate/a");
    let z = key("/count-gate/z");
    let m = key("/count-gate/m");
    let n = key("/count-gate/n");
    let a_result = missing();
    let z_result = missing();
    let left = PathObservationEpoch::from_shared([
        (a.dupe(), a_result.dupe()),
        (z.dupe(), z_result.dupe()),
    ])
    .unwrap();
    let right = PathObservationEpoch::from_shared([
        (a.dupe(), a_result.dupe()),
        (m.dupe(), missing()),
        (n.dupe(), missing()),
    ])
    .unwrap();
    assert!(right.observations().len() > left.observations().len());
    let (left_a, left_result) = left.observations().get_key_value(&a).unwrap();
    let (right_a, right_result) = right.observations().get_key_value(&a).unwrap();
    assert!(left_a.same_retention_owners(right_a));
    assert!(Arc::ptr_eq(left_result, right_result));
    assert!(right.get(&z).is_none());

    let actual = left.try_union(&right).unwrap();
    assert_eq!(actual, reference(&left, &right).unwrap());
    assert_eq!(
        actual.observations().keys().cloned().collect::<Vec<_>>(),
        vec![a, m, n, z.dupe()]
    );
    for (key, result) in left.observations() {
        let (retained_key, retained_result) = actual.observations().get_key_value(key).unwrap();
        assert!(retained_key.same_retention_owners(key));
        assert!(Arc::ptr_eq(retained_result, result));
        assert_eq!(key_hash(retained_key), key_hash(key));
    }
    assert!(Arc::ptr_eq(actual.get(&z).unwrap(), &z_result));
    // RHS cardinality and every overlap's custody qualify, but it omits z.
    // Count-based coverage must therefore force the complete growth fallback.
    assert!(!std::ptr::eq(actual.observations(), right.observations()));
    assert!(!std::ptr::eq(actual.observations(), left.observations()));
}
