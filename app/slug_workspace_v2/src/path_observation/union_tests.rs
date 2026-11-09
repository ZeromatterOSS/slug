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
