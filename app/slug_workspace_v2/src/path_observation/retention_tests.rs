//! Custody checks are stricter than structural identity and never retain credit.
use super::*;
use crate::ImageChargedPath;
use crate::ImageDescriptorBudget;
use crate::ImageObservationOperation;
use crate::ImageObservationRequest;

fn path(text: &str) -> NormalizedAbsolutePath {
    #[cfg(windows)]
    let text = format!("C:{text}");
    NormalizedAbsolutePath::new(text).unwrap()
}
fn hash(value: &PathObservationDemand) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
fn assert_equal_identity(left: &PathObservationDemand, right: &PathObservationDemand) {
    assert_eq!(left, right);
    assert_eq!(left.cmp(right), Ordering::Equal);
    assert_eq!(hash(left), hash(right));
    assert_eq!(format!("{left:?}"), format!("{right:?}"));
}

#[test]
fn demand_retention_distinguishes_shared_and_equal_independent_owners() {
    let target = path("/retention/member");
    let weak_path = Arc::downgrade(&target.path);
    for operation in [
        PathObservationOperation::Lstat,
        PathObservationOperation::ReadLink,
        PathObservationOperation::FileBytes,
        PathObservationOperation::FileDigest,
        PathObservationOperation::DirectoryEntries,
    ] {
        let left = PathObservationDemand::new(
            PathObservationNamespace::Host,
            target.dupe(),
            operation.dupe(),
        );
        let clone = left.dupe();
        assert!(left.same_retention_owners(&clone));
        let independent = PathObservationDemand::new(
            PathObservationNamespace::Host,
            NormalizedAbsolutePath::new(target.as_path().to_path_buf()).unwrap(),
            operation,
        );
        assert_equal_identity(&left, &independent);
        assert!(!Arc::ptr_eq(&left.path.path, &independent.path.path));
        assert!(!left.same_retention_owners(&independent));
        let other_namespace = PathObservationDemand::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
            target.dupe(),
            left.operation(),
        );
        assert!(!left.same_retention_owners(&other_namespace));
        assert!(other_namespace.same_retention_owners(&other_namespace.dupe()));
    }
    let lstat = PathObservationDemand::new(
        PathObservationNamespace::Host,
        target.dupe(),
        PathObservationOperation::Lstat,
    );
    let bytes = PathObservationDemand::new(
        PathObservationNamespace::Host,
        target.dupe(),
        PathObservationOperation::FileBytes,
    );
    assert!(!lstat.same_retention_owners(&bytes));
    drop((lstat, bytes));

    let raw: Arc<[u16]> = Arc::from([0x43, 0x3a, 0x5c, 0xd800]);
    let weak_raw = Arc::downgrade(&raw);
    let build = |option, raw| {
        if option {
            PathObservationDemand::windows_option_path_long_name(target.dupe(), raw)
        } else {
            PathObservationDemand::windows_long_path(target.dupe(), raw)
        }
    };
    for option in [false, true] {
        let left = build(option, raw.dupe());
        let clone = left.dupe();
        let independent = build(option, Arc::from(raw.as_ref()));
        assert_equal_identity(&left, &independent);
        assert!(Arc::ptr_eq(&left.path.path, &independent.path.path));
        assert!(!Arc::ptr_eq(
            left.windows_long_path_input.as_ref().unwrap(),
            independent.windows_long_path_input.as_ref().unwrap()
        ));
        assert!(!left.same_retention_owners(&independent));
        assert!(!left.same_retention_owners(&build(!option, raw.dupe())));
        let owners = (Arc::strong_count(&target.path), Arc::strong_count(&raw));
        for _ in 0..64 {
            assert!(left.same_retention_owners(&clone));
            assert!(!left.same_retention_owners(&independent));
        }
        assert_eq!(
            owners,
            (Arc::strong_count(&target.path), Arc::strong_count(&raw))
        );
    }
    drop(raw);
    assert!(weak_raw.upgrade().is_none());
    drop(target);
    assert!(weak_path.upgrade().is_none());
}

#[test]
fn image_retention_distinguishes_issued_and_request_anchor_blocks() {
    let bank = ImageDescriptorBudget::new(64 << 10);
    let foreign_bank = ImageDescriptorBudget::new(64 << 10);
    let target = path("/retention/member");
    let anchor_path = path("/retention");
    let weak_target = Arc::downgrade(&target.path);
    let weak_anchor = Arc::downgrade(&anchor_path.path);
    {
        let issued = ImageChargedPath::pin_existing(target.dupe(), &bank).unwrap();
        let anchor = ImageChargedPath::pin_existing(anchor_path.dupe(), &bank).unwrap();
        let request = ImageObservationRequest::new(anchor.dupe(), ImageObservationOperation::Lstat);
        let make = |issued, request| {
            PathObservationDemand::input_image(PathObservationNamespace::Host, issued, request)
                .unwrap()
        };
        let original = make(issued.dupe(), request.dupe());
        let clone = original.dupe();
        // Each wrapper keeps the SAME normalized Arc and SAME bank, but owns
        // an independently charged block. Neither may replace the original.
        let another_issued = issued.retain_charge(bank.try_reserve(7).unwrap()).unwrap();
        let another_anchor = anchor.retain_charge(bank.try_reserve(11).unwrap()).unwrap();
        assert!(another_issued.same_budget(&bank));
        assert!(another_anchor.same_budget(&bank));
        assert!(Arc::ptr_eq(
            &issued.normalized_ref().path,
            &another_issued.normalized_ref().path
        ));
        assert!(Arc::ptr_eq(
            &anchor.normalized_ref().path,
            &another_anchor.normalized_ref().path
        ));
        assert!(!issued.same_retention_owner(&another_issued));
        assert!(!anchor.same_retention_owner(&another_anchor));
        let issued_only = make(another_issued.dupe(), request.dupe());
        let another_request =
            ImageObservationRequest::new(another_anchor.dupe(), ImageObservationOperation::Lstat);
        assert!(!request.same_retention_owners(&another_request));
        assert!(request.same_retention_owners(&request.dupe()));
        let anchor_only = make(issued.dupe(), another_request);
        let foreign = make(
            ImageChargedPath::pin_existing(target.dupe(), &foreign_bank).unwrap(),
            ImageObservationRequest::new(
                ImageChargedPath::pin_existing(anchor_path.dupe(), &foreign_bank).unwrap(),
                ImageObservationOperation::Lstat,
            ),
        );
        for equal in [&issued_only, &anchor_only, &foreign] {
            assert_equal_identity(&original, equal);
            assert!(Arc::ptr_eq(&original.path.path, &equal.path.path));
            assert!(!original.same_retention_owners(equal));
            assert!(!equal.same_retention_owners(&original));
        }
        let different_operation = make(
            issued.dupe(),
            ImageObservationRequest::new(
                anchor.dupe(),
                ImageObservationOperation::BoundedDigest { max_bytes: 3 },
            ),
        );
        assert!(!original.same_retention_owners(&different_operation));
        let charged = (bank.used_bytes(), foreign_bank.used_bytes());
        let counts = (
            Arc::strong_count(&target.path),
            Arc::strong_count(&anchor_path.path),
        );
        assert!(charged.0 > 0 && charged.1 > 0);
        for _ in 0..64 {
            assert!(original.same_retention_owners(&clone));
            assert!(!original.same_retention_owners(&issued_only));
            assert!(!original.same_retention_owners(&anchor_only));
        }
        assert_eq!(charged, (bank.used_bytes(), foreign_bank.used_bytes()));
        assert_eq!(
            counts,
            (
                Arc::strong_count(&target.path),
                Arc::strong_count(&anchor_path.path)
            )
        );
    }
    assert_eq!(bank.used_bytes(), 0);
    assert_eq!(foreign_bank.used_bytes(), 0);
    drop((target, anchor_path));
    assert!(weak_target.upgrade().is_none());
    assert!(weak_anchor.upgrade().is_none());
}
