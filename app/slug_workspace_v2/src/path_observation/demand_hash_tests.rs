//! Operational demand hashing stays distinct from full path and fact identity.
use dice::DetectCycles;
use dice::Dice;
use starlark_map::StarlarkHasher;
use starlark_map::small_map::SmallMap;

use super::*;
use crate::ImageChargedPath;
use crate::ImageDescriptorBudget;
use crate::ImageObservationOperation;
use crate::ImageObservationRequest;

#[derive(Default)]
struct HashWork {
    writes: usize,
    bytes: usize,
}
impl Hasher for HashWork {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.writes += 1;
        self.bytes += bytes.len();
    }
}
fn path(text: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(text).unwrap()
}
fn plain(text: &str) -> PathObservationDemand {
    PathObservationDemand::new(
        PathObservationNamespace::Host,
        path(text),
        PathObservationOperation::Lstat,
    )
}
// Independent copy of the original derived Hash field stream, not the cache's
// production writer. No weak scalar enters generic NormalizedAbsolutePath Hash.
fn reference_hash(value: &PathObservationDemand, hasher: &mut impl Hasher) {
    value.namespace.hash(hasher);
    value.path.hash(hasher);
    value.operation.hash(hasher);
    value.windows_long_path_input.hash(hasher);
    value.image_path.hash(hasher);
}
fn fields(
    value: &PathObservationDemand,
) -> (
    &PathObservationNamespace,
    &NormalizedAbsolutePath,
    &PathObservationOperation,
    &Option<Arc<[u16]>>,
    &Option<ImageChargedPath>,
) {
    (
        &value.namespace,
        &value.path,
        &value.operation,
        &value.windows_long_path_input,
        &value.image_path,
    )
}
struct OriginalDebug<'a>(&'a PathObservationDemand);
impl fmt::Debug for OriginalDebug<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PathObservationDemand")
            .field("namespace", &self.0.namespace)
            .field("path", &self.0.path)
            .field("operation", &self.0.operation)
            .field("windows_long_path_input", &self.0.windows_long_path_input)
            .field("image_path", &self.0.image_path)
            .finish()
    }
}
fn image(
    bank: &ImageDescriptorBudget,
    anchor: &str,
    target: &str,
    operation: ImageObservationOperation,
) -> PathObservationDemand {
    let pin = |text| ImageChargedPath::pin_existing(path(text), bank).unwrap();
    PathObservationDemand::input_image(
        PathObservationNamespace::Host,
        pin(target),
        ImageObservationRequest::new(pin(anchor), operation),
    )
    .unwrap()
}
fn assert_cached(value: &PathObservationDemand) {
    let mut full = StarlarkHasher::new();
    reference_hash(value, &mut full);
    assert_eq!(value.cached_hash, full.finish());
    let mut work = HashWork::default();
    for _ in 0..64 {
        value.hash(&mut work);
    }
    assert_eq!(
        (work.writes, work.bytes),
        (64, 64 * std::mem::size_of::<u64>())
    );
    assert_eq!(format!("{value:?}"), format!("{:?}", OriginalDebug(value)));
    assert!(!format!("{value:?}").contains("cached_hash"));
}

#[test]
fn demand_cached_hash_binds_all_four_constructor_families_and_clones() {
    let bank = ImageDescriptorBudget::new(64 << 10);
    let values = [
        plain("/demand/a/../file"),
        PathObservationDemand::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(9)),
            path("/demand/file"),
            PathObservationOperation::FileBytes,
        ),
        PathObservationDemand::windows_long_path(
            path("/demand/file"),
            Arc::from([0x43, 0x3a, 0x5c, 0xd800]),
        ),
        PathObservationDemand::windows_option_path_long_name(
            path("/demand/file"),
            Arc::from([0x43, 0x3a, 0x5c, 0xd800]),
        ),
        image(
            &bank,
            "/demand",
            "/demand/file",
            ImageObservationOperation::Lstat,
        ),
    ];
    for value in &values {
        assert_cached(value);
        let clone = value.dupe();
        assert_cached(&clone);
        assert_eq!(value, &clone);
        assert_eq!(value.cmp(&clone), Ordering::Equal);
        assert_eq!(value.cached_hash, clone.cached_hash);
    }
    // Independent normalization-equivalent owners have equal structural keys.
    let foreign = plain("/demand/./file");
    assert!(!Arc::ptr_eq(&values[0].path.path, &foreign.path.path));
    assert_eq!(values[0], foreign);
    assert_eq!(values[0].cached_hash, foreign.cached_hash);
    let mut generic = HashWork::default();
    values[0].path.hash(&mut generic);
    let mut original = HashWork::default();
    values[0].path.as_path().hash(&mut original);
    assert_eq!(
        (generic.writes, generic.bytes),
        (original.writes, original.bytes),
        "generic path Hash retains its complete original field stream"
    );
    drop(values);
    assert_eq!(bank.used_bytes(), 0);
}

#[test]
fn demand_cache_collisions_preserve_original_five_field_equality_order_and_debug() {
    let bank = ImageDescriptorBudget::new(64 << 10);
    let other_bank = ImageDescriptorBudget::new(64 << 10);
    let mut values = vec![
        plain("/demand/a"),
        plain("/demand/b"),
        PathObservationDemand::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(1)),
            path("/demand/a"),
            PathObservationOperation::Lstat,
        ),
        PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/demand/a"),
            PathObservationOperation::FileBytes,
        ),
        PathObservationDemand::windows_long_path(path("/demand/a"), Arc::from([0x41, 0xd800])),
        PathObservationDemand::windows_long_path(path("/demand/a"), Arc::from([0x41, 0xd801])),
        PathObservationDemand::windows_option_path_long_name(
            path("/demand/a"),
            Arc::from([0x41, 0xd800]),
        ),
        image(
            &bank,
            "/demand",
            "/demand/sub/file",
            ImageObservationOperation::Lstat,
        ),
        image(
            &bank,
            "/demand/sub",
            "/demand/sub/file",
            ImageObservationOperation::Lstat,
        ),
        image(
            &bank,
            "/demand",
            "/demand/sub/file",
            ImageObservationOperation::BoundedDigest { max_bytes: 3 },
        ),
    ];
    // Isolate the fifth field in a private test corruption. No such inconsistent
    // selector can be created through the public checked constructor.
    let mut changed_image_field = values[7].dupe();
    changed_image_field.image_path =
        Some(ImageChargedPath::pin_existing(path("/demand/other"), &bank).unwrap());
    values.push(changed_image_field);
    for value in &mut values {
        value.cached_hash = 5;
    }
    let mut table = SmallMap::new();
    for (index, value) in values.iter().enumerate() {
        assert!(table.insert(value.dupe(), index).is_none());
        assert_eq!(format!("{value:?}"), format!("{:?}", OriginalDebug(value)));
        for other in &values {
            assert_eq!(value == other, fields(value) == fields(other));
            assert_eq!(value.cmp(other), fields(value).cmp(&fields(other)));
        }
    }
    assert_eq!(table.len(), values.len());
    for (index, value) in values.iter().enumerate() {
        assert_eq!(table.get(value), Some(&index));
    }
    let mut sorted = values.clone();
    sorted.sort();
    assert!(
        sorted
            .windows(2)
            .all(|pair| fields(&pair[0]) < fields(&pair[1]))
    );
    let mut foreign = image(
        &other_bank,
        "/demand",
        "/demand/sub/file",
        ImageObservationOperation::Lstat,
    );
    foreign.cached_hash = 5;
    assert_eq!(
        values[7], foreign,
        "independent charged owners are semantically equal"
    );
    assert_eq!(table.get(&foreign), Some(&7));
    drop(table);
    drop(sorted);
    drop(values);
    drop(foreign);
    assert_eq!((bank.used_bytes(), other_bank.used_bytes()), (0, 0));
}

#[tokio::test]
async fn actual_path_key_short_and_long_needs_hash_with_fixed_postconstruction_work() {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    for name in ["/a".to_owned(), format!("/{}/file", "segment/".repeat(128))] {
        let expected = plain(&name);
        let key = PathObservationKey::new(expected.dupe());
        let value = tx.compute(&key).await.unwrap();
        let PathOutcome::Need(need) = &value else {
            panic!("uninjected actual path key must request its original demand")
        };
        assert_eq!(need.demands(), &[expected]);
        assert!(!PathObservationKey::validity(&value));
        let actual = &need.demands()[0];
        assert_cached(actual);
        assert!(!PathObservationKey::equality(&value, &value));
        let mut key_work = HashWork::default();
        for _ in 0..64 {
            key.hash(&mut key_work);
        }
        assert_eq!((key_work.writes, key_work.bytes), (64, 512));
    }
}

#[test]
fn demand_inline_cache_layout_and_existing_payload_owners_retire_exactly() {
    #[allow(dead_code)]
    struct OriginalDemand {
        namespace: PathObservationNamespace,
        path: NormalizedAbsolutePath,
        operation: PathObservationOperation,
        windows_long_path_input: Option<Arc<[u16]>>,
        image_path: Option<ImageChargedPath>,
    }
    let old = std::mem::size_of::<OriginalDemand>();
    let new = std::mem::size_of::<PathObservationDemand>();
    let key = std::mem::size_of::<PathObservationKey>();
    let old_row = std::mem::size_of::<(OriginalDemand, Arc<PathObservationResult>)>();
    let new_row = std::mem::size_of::<(PathObservationDemand, Arc<PathObservationResult>)>();
    let delta = new.checked_sub(old).unwrap();
    println!(
        "DEMAND_HASH_LAYOUT legacy={old} demand={new} key={key} legacy_row={old_row} row={new_row} positive_delta={delta}"
    );
    assert_eq!(delta, std::mem::size_of::<u64>());
    assert_eq!(key, new);
    assert_eq!(new_row - old_row, delta);
    assert_eq!(
        std::mem::size_of::<NormalizedAbsolutePath>(),
        std::mem::size_of::<Arc<PathBuf>>()
    );
    let normalized = path("/demand/retained");
    let weak_path = Arc::downgrade(&normalized.path);
    let raw: Arc<[u16]> = Arc::from([0x41, 0xd800]);
    let weak_raw = Arc::downgrade(&raw);
    let original = PathObservationDemand::windows_long_path(normalized, raw);
    assert_eq!((weak_path.strong_count(), weak_raw.strong_count()), (1, 1));
    let key = PathObservationKey::new(original.dupe());
    assert_eq!((weak_path.strong_count(), weak_raw.strong_count()), (2, 2));
    drop(original);
    let need = NeedPathObservations::singleton(key.demand().dupe());
    drop(key);
    let last = need.dupe();
    drop(need);
    assert_eq!((weak_path.strong_count(), weak_raw.strong_count()), (1, 1));
    drop(last);
    assert!(weak_path.upgrade().is_none() && weak_raw.upgrade().is_none());

    let bank = ImageDescriptorBudget::new(64 << 10);
    let original = image(
        &bank,
        "/image",
        "/image/file",
        ImageObservationOperation::Lstat,
    );
    let retained = bank.used_bytes();
    let need = NeedPathObservations::singleton(original.dupe());
    assert_eq!(
        bank.used_bytes(),
        retained,
        "cached scalar adds no independent image owner"
    );
    drop(original);
    assert_eq!(bank.used_bytes(), retained);
    let last = need.dupe();
    drop(need);
    assert_eq!(bank.used_bytes(), retained);
    drop(last);
    assert_eq!(bank.used_bytes(), 0);
}
