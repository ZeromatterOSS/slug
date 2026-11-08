use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use dice::DetectCycles;
use dice::Dice;

use super::*;
use crate::PathIoErrorKind;
use crate::PathLstat;
use crate::PathObservationEpochKey;
use crate::PathObservationError;

type Entry = (PathObservationDemand, PathObservationResult);
fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}
fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
    PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
}
fn metadata(value: &str, kind: PathNodeKind, size: i64) -> Entry {
    (
        demand(value, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            kind, size, 2, 3, 4, 0o644,
        ))),
    )
}
fn digest(byte: u8) -> FileContentDigest {
    FileContentDigest::new([byte; 32], 1).unwrap()
}
fn script(target: &str, content: PathOperationResult<FileContentDigest>) -> Vec<Entry> {
    vec![
        metadata("/", PathNodeKind::Directory, 1),
        metadata("/entry", PathNodeKind::Symlink, 1),
        (
            demand("/entry", PathObservationOperation::ReadLink),
            PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(target.into()))),
        ),
        metadata(target, PathNodeKind::RegularFile, 1),
        (
            demand(target, PathObservationOperation::FileDigest),
            PathObservationResult::FileDigest(content),
        ),
    ]
}
fn epoch(script: &[Entry]) -> PathObservationEpoch {
    PathObservationEpoch::new(script.iter().cloned()).unwrap()
}
async fn update(transaction: dice::DiceTransaction, script: &[Entry]) -> dice::DiceTransaction {
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch(script))])
        .unwrap();
    updater.commit().await
}
fn keys() -> (PathFileDigestKey, PathFileDigestObservationKey) {
    (
        PathFileDigestKey::new(PathObservationNamespace::Host, path("/entry")),
        PathFileDigestObservationKey::new(PathObservationNamespace::Host, path("/entry")),
    )
}

#[derive(Debug, Clone, Allocative, Dupe)]
struct Count {
    key: PathFileDigestKey,
    count: Arc<AtomicUsize>,
}
impl PartialEq for Count {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key && Arc::ptr_eq(&self.count, &other.count)
    }
}
impl Eq for Count {}
impl std::hash::Hash for Count {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.key.hash(h);
        Arc::as_ptr(&self.count).hash(h);
    }
}
impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "count:{}", self.key)
    }
}
#[async_trait]
impl Key for Count {
    type Value = PathOutcome<usize>;
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        ctx.compute(&self.key)
            .await
            .unwrap()
            .map(|_| self.count.fetch_add(1, Ordering::SeqCst) + 1)
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[tokio::test]
async fn digest_need_frontier_and_semantic_cutoff() {
    let (key, observed_key) = keys();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let first_script = script("/a", PathOperationResult::Present(digest(1)));
    for prefix in 0..first_script.len() {
        tx = update(tx, &first_script[..prefix]).await;
        let value = tx.compute(&key).await.unwrap();
        let PathOutcome::Need(need) = &value else {
            panic!("prefix must Need: {value:?}")
        };
        assert_eq!(need.demands(), &[first_script[prefix].0.clone()]);
        assert!(!PathFileDigestKey::validity(&value));
        assert!(!PathFileDigestKey::equality(&value, &value));
        let observed = tx.compute(&observed_key).await.unwrap();
        assert!(!PathFileDigestObservationKey::validity(&observed));
        assert!(!PathFileDigestObservationKey::equality(
            &observed, &observed
        ));
    }
    tx = update(tx, &first_script).await;
    let first = tx.compute(&observed_key).await.unwrap();
    let PathOutcome::Complete(Ok(observed)) = &first else {
        panic!("{first:?}")
    };
    assert_eq!(observed.observations(), &epoch(&first_script));
    assert_eq!(observed.result(), &Ok(PathFileDigest::Present(digest(1))));
    let counter = Count {
        key: key.clone(),
        count: Arc::new(AtomicUsize::new(0)),
    };
    assert_complete_eq(
        tx.compute(&counter).await.unwrap(),
        PathOutcome::Complete(1),
    );
    let mut retarget = script("/b", PathOperationResult::Present(digest(1)));
    retarget[3] = (
        retarget[3].0.clone(),
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            PathNodeKind::RegularFile,
            1,
            90,
            91,
            92,
            0o755,
        ))),
    );
    tx = update(tx, &retarget).await;
    assert!(!tx.compute(&observed_key).await.unwrap().complete_eq(&first));
    assert_complete_eq(
        tx.compute(&counter).await.unwrap(),
        PathOutcome::Complete(1),
    );
    tx = update(tx, &script("/b", PathOperationResult::Present(digest(2)))).await;
    assert_complete_eq(
        tx.compute(&key).await.unwrap(),
        PathOutcome::Complete(Ok(PathFileDigest::Present(digest(2)))),
    );
    assert_complete_eq(
        tx.compute(&counter).await.unwrap(),
        PathOutcome::Complete(2),
    );
    tx = update(tx, &first_script).await;
    assert_complete_eq(tx.compute(&observed_key).await.unwrap(), first.clone());
    assert_complete_eq(
        tx.compute(&counter).await.unwrap(),
        PathOutcome::Complete(3),
    );
    let deleted = vec![
        metadata("/", PathNodeKind::Directory, 1),
        (
            demand("/entry", PathObservationOperation::Lstat),
            PathObservationResult::Lstat(PathOperationResult::Missing),
        ),
    ];
    tx = update(tx, &deleted).await;
    assert_complete_eq(
        tx.compute(&key).await.unwrap(),
        PathOutcome::Complete(Ok(PathFileDigest::Missing)),
    );
    assert_complete_eq(
        tx.compute(&counter).await.unwrap(),
        PathOutcome::Complete(4),
    );
    tx = update(tx, &first_script).await;
    assert_complete_eq(tx.compute(&observed_key).await.unwrap(), first.clone());
    assert_complete_eq(
        tx.compute(&counter).await.unwrap(),
        PathOutcome::Complete(5),
    );
}

#[tokio::test]
async fn digest_errors_keep_their_observed_frontier() {
    let (key, observed_key) = keys();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let denied = PathObservationError::Io {
        kind: PathIoErrorKind::PermissionDenied,
        raw_os_error: Some(13),
    };
    for (content, expected) in [
        (
            PathOperationResult::Missing,
            PathFileDigestError::Read(PathFileBytesError::InconsistentState {
                logical_path: path("/entry"),
                operation: PathObservationOperation::FileDigest,
                before: Some(PathLstat::new(PathNodeKind::RegularFile, 1, 2, 3, 4, 0o644)),
                after: None,
            }),
        ),
        (
            PathOperationResult::Error(denied),
            PathFileDigestError::Read(PathFileBytesError::Observation {
                logical_path: path("/entry"),
                operation: PathObservationOperation::FileDigest,
                error: denied,
            }),
        ),
        (
            PathOperationResult::Present(FileContentDigest::new([3; 32], 2).unwrap()),
            PathFileDigestError::SizeChanged {
                logical_path: path("/entry"),
                expected: 1,
                observed: 2,
            },
        ),
    ] {
        let entries = script("/a", content);
        tx = update(tx, &entries).await;
        assert_complete_eq(
            tx.compute(&key).await.unwrap(),
            PathOutcome::Complete(Err(expected.clone())),
        );
        let PathOutcome::Complete(Ok(observed)) = tx.compute(&observed_key).await.unwrap() else {
            panic!()
        };
        assert_eq!(observed.result(), &Err(expected));
        assert_eq!(observed.observations(), &epoch(&entries));
    }
    for kind in [PathNodeKind::Directory, PathNodeKind::SpecialFile] {
        let entries = [
            metadata("/", PathNodeKind::Directory, 1),
            metadata("/entry", kind, 1),
        ];
        tx = update(tx, &entries).await;
        assert_complete_eq(
            tx.compute(&key).await.unwrap(),
            PathOutcome::Complete(Err(PathFileDigestError::Read(
                PathFileBytesError::WrongKind {
                    logical_path: path("/entry"),
                    expected: PathNodeKind::RegularFile,
                    actual: kind,
                },
            ))),
        );
    }
    let mut cycle = script("/entry", PathOperationResult::Present(digest(1)));
    cycle.truncate(3);
    tx = update(tx, &cycle).await;
    assert_complete_eq(
        tx.compute(&key).await.unwrap(),
        PathOutcome::Complete(Err(PathFileDigestError::Read(PathFileBytesError::Cycle {
            logical_path: path("/entry"),
        }))),
    );
}

#[test]
fn digest_size_domain_and_fixed_retention() {
    assert!(FileContentDigest::new([0; 32], 0).is_some());
    assert!(FileContentDigest::new([0; 32], i64::MAX as u64).is_some());
    assert!(FileContentDigest::new([0; 32], i64::MAX as u64 + 1).is_none());
    assert_eq!(std::mem::size_of::<FileContentDigest>(), 40);
}

fn assert_complete_eq<T: PartialEq + std::fmt::Debug>(
    actual: PathOutcome<T>,
    expected: PathOutcome<T>,
) {
    assert!(
        actual.complete_eq(&expected),
        "actual: {actual:?}; expected: {expected:?}"
    );
}

#[tokio::test]
async fn anchored_digest_never_demands_host_ancestors_or_relabels_equal_display_instances() {
    use crate::PathObservationInstanceId;
    let anchor = |instance| {
        MaterializationPathAnchor::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance)),
            path("/same"),
        )
        .unwrap()
    };
    let a = anchor(501);
    let b = anchor(502);
    let key_a = AnchoredPathFileDigestObservationKey::new(a.dupe(), "/same/file").unwrap();
    let key_b = AnchoredPathFileDigestObservationKey::new(b.dupe(), "/same/file").unwrap();
    assert_ne!(key_a, key_b);
    assert!(AnchoredPathFileDigestObservationKey::new(a.dupe(), "/same/../outside").is_err());
    let rows = |anchor: &MaterializationPathAnchor, value: u8| {
        let demand =
            |name, operation| PathObservationDemand::new(anchor.namespace(), path(name), operation);
        vec![
            (
                demand("/same", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::Directory,
                    0,
                    1,
                    1,
                    1,
                    0o755,
                ))),
            ),
            (
                demand("/same/file", PathObservationOperation::Lstat),
                PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
                    PathNodeKind::RegularFile,
                    1,
                    1,
                    1,
                    1,
                    0o644,
                ))),
            ),
            (
                demand("/same/file", PathObservationOperation::FileDigest),
                PathObservationResult::FileDigest(PathOperationResult::Present(digest(value))),
            ),
        ]
    };
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to([(PathObservationEpochKey, PathObservationEpoch::empty())])
        .unwrap();
    let mut tx = updater.commit().await;
    let PathOutcome::Need(need) = tx.compute(&key_a).await.unwrap() else {
        panic!("empty anchor epoch must need root")
    };
    assert_eq!(need.demands()[0].path(), a.root());
    assert_eq!(need.demands()[0].namespace(), a.namespace());
    assert!(!AnchoredPathFileDigestObservationKey::validity(
        &PathOutcome::Need(need)
    ));
    let mut updater = tx.into_updater();
    updater
        .changed_to([(PathObservationEpochKey, epoch(&rows(&a, 1)))])
        .unwrap();
    tx = updater.commit().await;
    let PathOutcome::Complete(Ok(first)) = tx.compute(&key_a).await.unwrap() else {
        panic!("A must complete without ancestors")
    };
    assert_eq!(first.result(), &Ok(PathFileDigest::Present(digest(1))));
    assert!(
        first
            .observations()
            .observations()
            .keys()
            .all(|d| d.path().as_path().starts_with(a.root().as_path()))
    );
    assert!(matches!(
        tx.compute(&key_b).await.unwrap(),
        PathOutcome::Need(_)
    ));
    let mut updater = tx.into_updater();
    updater
        .changed_to([(PathObservationEpochKey, epoch(&rows(&b, 2)))])
        .unwrap();
    tx = updater.commit().await;
    let PathOutcome::Complete(Ok(second)) = tx.compute(&key_b).await.unwrap() else {
        panic!("B must complete")
    };
    assert_eq!(second.result(), &Ok(PathFileDigest::Present(digest(2))));
    assert!(matches!(
        tx.compute(&key_a).await.unwrap(),
        PathOutcome::Need(_)
    ));
    let mut updater = tx.into_updater();
    updater
        .changed_to([(PathObservationEpochKey, epoch(&rows(&a, 1)))])
        .unwrap();
    tx = updater.commit().await;
    let PathOutcome::Complete(Ok(restored)) = tx.compute(&key_a).await.unwrap() else {
        panic!("A restore must complete")
    };
    assert_eq!(restored.result(), first.result());
}
