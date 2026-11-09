use dice::DetectCycles;
use dice::Dice;

use super::*;
use crate::ImageChargedPath;
use crate::ImageDescriptorBudget;
use crate::ImageObservationData;
use crate::ImageObservationOperation;
use crate::ImageObservationRequest;
use crate::ImageObservationResult;

fn path(text: &str) -> NormalizedAbsolutePath {
    #[cfg(windows)]
    let text = format!("C:{text}");
    NormalizedAbsolutePath::new(text).unwrap()
}

#[derive(Default)]
struct HashStream(Vec<u8>);
impl Hasher for HashStream {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}

#[tokio::test]
async fn actual_path_need_shared_owners_skip_all_full_order_comparisons() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    for name in ["/a".to_owned(), format!("/{}/file", "segment/".repeat(128))] {
        let original = PathObservationDemand::new(
            PathObservationNamespace::Host,
            path(&name),
            PathObservationOperation::Lstat,
        );
        let key = PathObservationKey::new(original.dupe());
        let value = tx.compute(&key).await.unwrap();
        let PathOutcome::Need(need) = &value else {
            panic!("uninjected actual path key must return its demand");
        };
        assert_eq!(need.demands(), &[original.dupe()]);
        assert!(!PathObservationKey::validity(&value));
        assert!(!PathObservationKey::equality(&value, &value));
        let actual = &need.demands()[0];
        assert!(Arc::ptr_eq(&actual.path.path, &original.path.path));
        let duplicate = actual.dupe();
        assert_eq!(key, PathObservationKey::new(duplicate.dupe()));

        // TLS sampling is one synchronous closed interval after the DICE await.
        reset_path_order_fallbacks_for_test();
        for _ in 0..64 {
            assert_eq!(actual.path.cmp(&duplicate.path), Ordering::Equal);
        }
        assert_eq!(path_order_fallbacks_for_test(), 0);
        reset_path_order_fallbacks_for_test();
        for _ in 0..64 {
            assert_eq!(actual.cmp(&duplicate), Ordering::Equal);
        }
        assert_eq!(path_order_fallbacks_for_test(), 0);
    }
}

#[test]
fn independent_path_owners_keep_exact_platform_order_and_full_hash_stream() {
    let names = ["/", "/a", "/a/./b/..", "/a/b", "/a/bb", "/λ/file"];
    for left in names {
        for right in names {
            let a = path(left);
            let b = path(right);
            assert!(!Arc::ptr_eq(&a.path, &b.path));
            let expected = a.path.as_ref().cmp(b.path.as_ref());
            assert_eq!(a == b, a.as_path() == b.as_path());
            reset_path_order_fallbacks_for_test();
            for _ in 0..32 {
                assert_eq!(a.cmp(&b), expected);
                assert_eq!(a.partial_cmp(&b), Some(expected));
            }
            assert_eq!(path_order_fallbacks_for_test(), 64);
            let mut actual = HashStream::default();
            let mut original = HashStream::default();
            a.hash(&mut actual);
            a.path.as_ref().hash(&mut original);
            assert_eq!(actual.0, original.0);
            assert_eq!(
                format!("{a:?}"),
                format!("NormalizedAbsolutePath {{ path: {:?} }}", a.path)
            );
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;

        let a = NormalizedAbsolutePath::new(PathBuf::from(OsString::from_vec(
            b"/non-unicode/\xff".to_vec(),
        )))
        .unwrap();
        let duplicate = a.dupe();
        assert!(a.as_path().to_str().is_none());
        reset_path_order_fallbacks_for_test();
        for _ in 0..64 {
            assert_eq!(a.cmp(&duplicate), Ordering::Equal);
        }
        assert_eq!(path_order_fallbacks_for_test(), 0);
        for raw in [b"/non-unicode/\xff", b"/non-unicode/\xfe"] {
            let b = NormalizedAbsolutePath::new(PathBuf::from(OsString::from_vec(raw.to_vec())))
                .unwrap();
            assert!(!Arc::ptr_eq(&a.path, &b.path));
            let expected = a.path.as_ref().cmp(b.path.as_ref());
            reset_path_order_fallbacks_for_test();
            for _ in 0..64 {
                assert_eq!(a.cmp(&b), expected);
            }
            assert_eq!(path_order_fallbacks_for_test(), 64);
            assert_eq!(a == b, a.as_path() == b.as_path());
            let mut actual = HashStream::default();
            let mut original = HashStream::default();
            b.hash(&mut actual);
            b.path.as_ref().hash(&mut original);
            assert_eq!(actual.0, original.0);
        }
    }
    assert_eq!(
        std::mem::size_of::<NormalizedAbsolutePath>(),
        std::mem::size_of::<Arc<PathBuf>>()
    );
    let normalized = path("/retained/spare/capacity");
    assert_eq!(
        normalized.retained_storage_bytes(),
        Some(
            2 * std::mem::size_of::<usize>()
                + std::mem::size_of::<PathBuf>()
                + normalized.path.capacity()
        )
    );
    let weak = Arc::downgrade(&normalized.path);
    let duplicate = normalized.dupe();
    let owners = weak.strong_count();
    let storage = normalized.retained_storage_bytes();
    for _ in 0..64 {
        assert_eq!(normalized.cmp(&duplicate), Ordering::Equal);
    }
    assert_eq!(weak.strong_count(), owners);
    assert_eq!(normalized.retained_storage_bytes(), storage);
    drop(normalized);
    assert!(weak.upgrade().is_some());
    drop(duplicate);
    assert!(weak.upgrade().is_none());
}

fn image_row(
    bank: &ImageDescriptorBudget,
    target: &NormalizedAbsolutePath,
    operation: ImageObservationOperation,
) -> (PathObservationDemand, Arc<PathObservationResult>) {
    let issued = ImageChargedPath::pin_existing(target.dupe(), bank).unwrap();
    let anchor = ImageChargedPath::pin_existing(path("/image"), bank).unwrap();
    let request = ImageObservationRequest::new(anchor, operation);
    let demand = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        issued.dupe(),
        request.dupe(),
    )
    .unwrap();
    let result = Arc::new(PathObservationResult::InputImage(
        ImageObservationResult::new(
            issued,
            request,
            ImageObservationData::Lstat(PathOperationResult::Missing),
            bank.try_reserve(64).unwrap(),
        )
        .unwrap(),
    ));
    (demand, result)
}

#[test]
fn shared_path_order_preserves_other_demand_fields_and_image_custody() {
    let target = path("/image/member");
    let weak_path = Arc::downgrade(&target.path);
    let raw: Arc<[u16]> = Arc::from([0x41, 0xd800]);
    let weak_raw = Arc::downgrade(&raw);
    let bank_a = ImageDescriptorBudget::new(64 << 10);
    let bank_b = ImageDescriptorBudget::new(64 << 10);
    let (image_a, result_a) = image_row(&bank_a, &target, ImageObservationOperation::Lstat);
    let (image_b, result_b) = image_row(&bank_b, &target, ImageObservationOperation::Lstat);
    assert_eq!(image_a, image_b);
    assert_eq!(result_a, result_b);
    assert!(!Arc::ptr_eq(&result_a, &result_b));
    let weak_result_a = Arc::downgrade(&result_a);
    let weak_result_b = Arc::downgrade(&result_b);
    let digest_image = PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        ImageChargedPath::pin_existing(target.dupe(), &bank_a).unwrap(),
        ImageObservationRequest::new(
            ImageChargedPath::pin_existing(path("/image"), &bank_a).unwrap(),
            ImageObservationOperation::BoundedDigest { max_bytes: 3 },
        ),
    )
    .unwrap();
    let values = [
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            target.dupe(),
            PathObservationOperation::Lstat,
        ),
        PathObservationDemand::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
            target.dupe(),
            PathObservationOperation::Lstat,
        ),
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            target.dupe(),
            PathObservationOperation::FileBytes,
        ),
        PathObservationDemand::windows_long_path(target.dupe(), raw),
        PathObservationDemand::windows_long_path(target.dupe(), Arc::from([0x41, 0xd801])),
        PathObservationDemand::windows_option_path_long_name(
            target.dupe(),
            Arc::from([0x41, 0xd800]),
        ),
        image_a.dupe(),
        digest_image,
    ];
    let fields = |value: &PathObservationDemand| {
        (
            value.namespace,
            value.path.as_path().to_path_buf(),
            value.operation.dupe(),
            value.windows_long_path_input.dupe(),
            value.image_path.dupe(),
        )
    };
    for (index, left) in values.iter().enumerate() {
        assert!(Arc::ptr_eq(&left.path.path, &target.path));
        for (other_index, right) in values.iter().enumerate() {
            assert_eq!(left.cmp(right), fields(left).cmp(&fields(right)));
            assert_eq!(left == right, index == other_index);
        }
    }
    let a = PathObservationEpoch::from_shared([(image_a, result_a)]).unwrap();
    let b = PathObservationEpoch::from_shared([(image_b, result_b)]).unwrap();
    let union = a.try_union(&b).unwrap();
    assert!(std::ptr::eq(union.observations(), a.observations()));
    drop(values);
    assert!(weak_raw.upgrade().is_none());
    let charged = (bank_a.used_bytes(), bank_b.used_bytes());
    drop(target);
    assert_eq!((bank_a.used_bytes(), bank_b.used_bytes()), charged);
    drop(a);
    drop(b);
    assert_eq!(bank_a.used_bytes(), charged.0);
    assert_eq!(bank_b.used_bytes(), 0);
    assert!(weak_result_a.upgrade().is_some());
    assert!(weak_result_b.upgrade().is_none());
    assert!(weak_path.upgrade().is_some());
    drop(union);
    assert_eq!((bank_a.used_bytes(), bank_b.used_bytes()), (0, 0));
    assert!(weak_result_a.upgrade().is_none());
    assert!(weak_path.upgrade().is_none());
}
