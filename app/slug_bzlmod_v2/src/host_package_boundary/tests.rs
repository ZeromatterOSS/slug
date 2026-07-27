#[cfg(unix)]
use std::fmt;
#[cfg(unix)]
use std::hash::Hash;
#[cfg(unix)]
use std::hash::Hasher;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::Arc;
#[cfg(unix)]
use std::sync::Mutex;
#[cfg(unix)]
use std::sync::atomic::AtomicUsize;
#[cfg(unix)]
use std::sync::atomic::Ordering;

#[cfg(unix)]
use allocative::Allocative;
#[cfg(unix)]
use async_trait::async_trait;
#[cfg(unix)]
use dice::ActivationData;
#[cfg(unix)]
use dice::ActivationTracker;
#[cfg(unix)]
use dice::DetectCycles;
#[cfg(unix)]
use dice::Dice;
#[cfg(unix)]
use dice::DiceComputations;
#[cfg(unix)]
use dice::DiceTransaction;
#[cfg(unix)]
use dice::DynKey;
#[cfg(unix)]
use dice::Key;
#[cfg(unix)]
use dice::UserComputationData;
#[cfg(unix)]
use dice_futures::cancellation::CancellationContext;
#[cfg(unix)]
use dupe::Dupe;
#[cfg(unix)]
use slug_identity_v2::PackagePath;
#[cfg(unix)]
use slug_workspace_v2::NormalizedAbsolutePath;
#[cfg(unix)]
use slug_workspace_v2::PathIoErrorKind;
#[cfg(unix)]
use slug_workspace_v2::PathLstat;
#[cfg(unix)]
use slug_workspace_v2::PathNodeKind;
#[cfg(unix)]
use slug_workspace_v2::PathObservationDemand;
#[cfg(unix)]
use slug_workspace_v2::PathObservationEpoch;
#[cfg(unix)]
use slug_workspace_v2::PathObservationEpochKey;
#[cfg(unix)]
use slug_workspace_v2::PathObservationError;
#[cfg(unix)]
use slug_workspace_v2::PathObservationNamespace;
#[cfg(unix)]
use slug_workspace_v2::PathObservationOperation;
#[cfg(unix)]
use slug_workspace_v2::PathObservationResult;
#[cfg(unix)]
use slug_workspace_v2::PathOperationResult;
#[cfg(unix)]
use slug_workspace_v2::PathOutcome;

#[cfg(unix)]
use super::HostRootPackageBoundaryErrorInner;
#[cfg(unix)]
use super::HostRootPackageBoundaryKey;
#[cfg(unix)]
use super::HostRootPackageBoundaryKind;
#[cfg(unix)]
use crate::RootPackagePolicyInputs;
#[cfg(unix)]
use crate::host_package::HostRootPackageLookupKey;
#[cfg(unix)]
use crate::inject_root_package_policy_inputs;
#[cfg(unix)]
use crate::repository_ignore::HostRepositoryIgnoreKey;
#[cfg(unix)]
type ScriptEntry = (PathObservationDemand, PathObservationResult);

#[cfg(unix)]
#[derive(Default)]
struct BoundaryTracker {
    boundary_dependencies: Mutex<Vec<Vec<String>>>,
    lookup_activations: AtomicUsize,
    activated: Mutex<Vec<String>>,
}

#[cfg(unix)]
impl ActivationTracker for BoundaryTracker {
    fn key_activated(
        &self,
        key: &DynKey,
        dependencies: &mut dyn Iterator<Item = &DynKey>,
        _activation: ActivationData,
    ) {
        self.activated.lock().unwrap().push(key.to_string());
        if key.downcast_ref::<HostRootPackageLookupKey>().is_some() {
            self.lookup_activations.fetch_add(1, Ordering::SeqCst);
        }
        if key.downcast_ref::<HostRootPackageBoundaryKey>().is_some() {
            self.boundary_dependencies
                .lock()
                .unwrap()
                .push(dependencies.map(ToString::to_string).collect());
        }
    }
}

#[cfg(unix)]
fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(value).unwrap()
}

#[cfg(unix)]
fn lstat(kind: PathNodeKind, variant: i64) -> PathLstat {
    PathLstat::new(kind, variant, variant, variant, variant, 0o755)
}

#[cfg(unix)]
fn demand(value: &str, operation: PathObservationOperation) -> PathObservationDemand {
    PathObservationDemand::new(PathObservationNamespace::Host, path(value), operation)
}

#[cfg(unix)]
fn observed_lstat(value: &str, result: PathOperationResult<PathLstat>) -> ScriptEntry {
    (
        demand(value, PathObservationOperation::Lstat),
        PathObservationResult::Lstat(result),
    )
}

#[cfg(unix)]
fn present(value: &str, kind: PathNodeKind, variant: i64) -> ScriptEntry {
    observed_lstat(value, PathOperationResult::Present(lstat(kind, variant)))
}

#[cfg(unix)]
fn missing(value: &str) -> ScriptEntry {
    observed_lstat(value, PathOperationResult::Missing)
}

#[cfg(unix)]
fn lstat_error(value: &str) -> ScriptEntry {
    observed_lstat(
        value,
        PathOperationResult::Error(PathObservationError::Io {
            kind: PathIoErrorKind::PermissionDenied,
            raw_os_error: Some(13),
        }),
    )
}

#[cfg(unix)]
fn bytes(value: &str, contents: &'static [u8]) -> ScriptEntry {
    (
        demand(value, PathObservationOperation::FileBytes),
        PathObservationResult::FileBytes(PathOperationResult::Present(Arc::from(contents))),
    )
}

#[cfg(unix)]
fn read_link(value: &str, target: &str) -> ScriptEntry {
    (
        demand(value, PathObservationOperation::ReadLink),
        PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
            target,
        )))),
    )
}

#[cfg(unix)]
fn inputs(roots: &[&str], deleted: &[&str]) -> RootPackagePolicyInputs {
    RootPackagePolicyInputs::new(
        path("/workspace"),
        roots.iter().map(|root| path(root)).collect::<Vec<_>>(),
        deleted,
        None,
        Some("warning"),
    )
    .unwrap()
}

#[cfg(unix)]
fn repository_prelude(roots: &[&str], variant: i64) -> Vec<ScriptEntry> {
    let mut entries = vec![
        present("/", PathNodeKind::Directory, variant),
        present("/workspace", PathNodeKind::Directory, variant),
        missing("/workspace/REPO.bazel"),
    ];
    for root in roots {
        entries.push(present(root, PathNodeKind::Directory, variant));
        entries.push(missing(&format!("{root}/.bazelignore")));
    }
    entries
}

#[cfg(unix)]
fn epoch(entries: &[ScriptEntry]) -> PathObservationEpoch {
    PathObservationEpoch::new(
        entries
            .iter()
            .map(|(demand, result)| (demand.dupe(), result.dupe())),
    )
    .unwrap()
}

#[cfg(unix)]
async fn boundary(
    policy: Option<RootPackagePolicyInputs>,
    entries: Vec<ScriptEntry>,
    package: &str,
) -> PathOutcome<Arc<Result<super::HostRootPackageBoundary, super::HostRootPackageBoundaryError>>> {
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    if let Some(policy) = policy {
        inject_root_package_policy_inputs(&mut updater, policy).unwrap();
    }
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(entries).unwrap(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    transaction
        .compute(&HostRootPackageBoundaryKey::new(
            path("/workspace"),
            PackagePath::parse(package).unwrap(),
        ))
        .await
        .unwrap()
}

#[cfg(unix)]
async fn tracked_boundary(
    policy: RootPackagePolicyInputs,
    entries: Vec<ScriptEntry>,
    package: &str,
) -> (
    PathOutcome<Arc<Result<super::HostRootPackageBoundary, super::HostRootPackageBoundaryError>>>,
    Arc<BoundaryTracker>,
) {
    let tracker = Arc::new(BoundaryTracker::default());
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater_with_data(UserComputationData {
        activation_tracker: Some(tracker.dupe() as Arc<dyn ActivationTracker>),
        ..Default::default()
    });
    inject_root_package_policy_inputs(&mut updater, policy).unwrap();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch(&entries))])
        .unwrap();
    let mut transaction = updater.commit().await;
    let outcome = transaction
        .compute(&HostRootPackageBoundaryKey::new(
            path("/workspace"),
            PackagePath::parse(package).unwrap(),
        ))
        .await
        .unwrap();
    (outcome, tracker)
}

#[cfg(unix)]
fn complete(
    value: &PathOutcome<
        Arc<Result<super::HostRootPackageBoundary, super::HostRootPackageBoundaryError>>,
    >,
) -> &super::HostRootPackageBoundary {
    let PathOutcome::Complete(value) = value else {
        panic!("expected a complete boundary result");
    };
    value
        .as_ref()
        .as_ref()
        .expect("expected a successful boundary")
}

#[cfg(unix)]
#[derive(Debug, Clone, Allocative)]
struct BoundaryCounterKey {
    boundary: HostRootPackageBoundaryKey,
    #[allocative(skip)]
    counter: Arc<AtomicUsize>,
}

#[cfg(unix)]
impl PartialEq for BoundaryCounterKey {
    fn eq(&self, other: &Self) -> bool {
        self.boundary == other.boundary && Arc::ptr_eq(&self.counter, &other.counter)
    }
}

#[cfg(unix)]
impl Eq for BoundaryCounterKey {}

#[cfg(unix)]
impl Hash for BoundaryCounterKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.boundary.hash(state);
        Arc::as_ptr(&self.counter).hash(state);
    }
}

#[cfg(unix)]
impl fmt::Display for BoundaryCounterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "host-package-boundary-counter:{}", self.boundary)
    }
}

#[cfg(unix)]
#[async_trait]
impl Key for BoundaryCounterKey {
    type Value = PathOutcome<usize>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        ctx.compute(&self.boundary)
            .await
            .unwrap()
            .map(|_| self.counter.fetch_add(1, Ordering::SeqCst) + 1)
    }
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x.complete_eq(y)
    }
    fn validity(value: &Self::Value) -> bool {
        value.is_complete()
    }
}

#[cfg(unix)]
async fn update_epoch(transaction: DiceTransaction, entries: &[ScriptEntry]) -> DiceTransaction {
    let mut updater = transaction.into_updater();
    updater
        .changed_to(vec![(PathObservationEpochKey, epoch(entries))])
        .unwrap();
    updater.commit().await
}

#[cfg(unix)]
async fn update_policy(
    transaction: DiceTransaction,
    policy: RootPackagePolicyInputs,
) -> DiceTransaction {
    let mut updater = transaction.into_updater();
    inject_root_package_policy_inputs(&mut updater, policy).unwrap();
    updater.commit().await
}

#[cfg(unix)]
#[test]
fn key_identity_and_display_are_only_workspace_and_package() {
    let pkg = PackagePath::parse("pkg/child").unwrap();
    let first = HostRootPackageBoundaryKey::new(path("/workspace"), pkg.clone());
    let same = HostRootPackageBoundaryKey::new(path("/workspace"), pkg);
    let other = HostRootPackageBoundaryKey::new(
        path("/other-workspace"),
        PackagePath::parse("pkg/child").unwrap(),
    );
    assert_eq!(first, same);
    assert_ne!(first, other);
    assert_eq!(
        first.to_string(),
        "host-root-package-boundary:\"/workspace\"//pkg/child"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn projects_all_four_kinds_and_retains_only_selected_root() {
    let roots = ["/root-a", "/root-b"];
    let mut no_package_entries = repository_prelude(&roots, 1);
    for root in roots {
        no_package_entries.extend([
            present(&format!("{root}/ordinary"), PathNodeKind::Directory, 1),
            missing(&format!("{root}/ordinary/BUILD.bazel")),
            missing(&format!("{root}/ordinary/BUILD")),
        ]);
    }
    let no_package = boundary(Some(inputs(&roots, &[])), no_package_entries, "ordinary").await;
    assert_eq!(
        complete(&no_package).kind(),
        HostRootPackageBoundaryKind::NoPackage
    );
    assert_eq!(complete(&no_package).selected_package_root(), None);

    let mut deleted_entries = repository_prelude(&roots, 2);
    deleted_entries.extend([
        present("/root-a/deleted", PathNodeKind::Directory, 2),
        present("/root-a/deleted/BUILD.bazel", PathNodeKind::RegularFile, 2),
    ]);
    let deleted = boundary(
        Some(inputs(&roots, &["//deleted"])),
        deleted_entries,
        "deleted",
    )
    .await;
    assert_eq!(
        complete(&deleted).kind(),
        HostRootPackageBoundaryKind::DeletedPackage
    );
    assert_eq!(complete(&deleted).selected_package_root(), None);

    let mut ignored_entries = repository_prelude(&roots, 3);
    ignored_entries.retain(|(demand, _)| {
        demand.path().as_path() != std::path::Path::new("/root-a/.bazelignore")
    });
    ignored_entries.push(present(
        "/root-a/.bazelignore",
        PathNodeKind::RegularFile,
        3,
    ));
    ignored_entries.push(bytes("/root-a/.bazelignore", b"ignored\n"));
    let ignored = boundary(Some(inputs(&roots, &[])), ignored_entries, "ignored/child").await;
    assert_eq!(
        complete(&ignored).kind(),
        HostRootPackageBoundaryKind::IgnoredDirectory
    );
    assert_eq!(complete(&ignored).selected_package_root(), None);

    let mut package_entries = repository_prelude(&roots, 4);
    package_entries.extend([
        present("/root-a/pkg", PathNodeKind::Directory, 4),
        missing("/root-a/pkg/BUILD.bazel"),
        present("/root-a/pkg/BUILD", PathNodeKind::RegularFile, 4),
    ]);
    let package = boundary(Some(inputs(&roots, &[])), package_entries, "pkg").await;
    assert_eq!(
        complete(&package).kind(),
        HostRootPackageBoundaryKind::Package
    );
    assert_eq!(
        complete(&package).selected_package_root(),
        Some(&path("/root-a"))
    );
    assert_eq!(
        format!("{:?}", complete(&package)),
        "HostRootPackageBoundary"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn selection_uses_root_major_marker_minor_and_file_semantics() {
    let roots = ["/root-a", "/root-b"];
    let mut entries = repository_prelude(&roots, 1);
    entries.extend([
        present("/root-a/pkg", PathNodeKind::Directory, 1),
        missing("/root-a/pkg/BUILD.bazel"),
        present("/root-a/pkg/BUILD", PathNodeKind::RegularFile, 1),
        present("/root-b/pkg", PathNodeKind::Directory, 1),
        present("/root-b/pkg/BUILD.bazel", PathNodeKind::RegularFile, 1),
    ]);
    let selected = boundary(Some(inputs(&roots, &[])), entries, "pkg").await;
    assert_eq!(
        complete(&selected).selected_package_root(),
        Some(&path("/root-a"))
    );

    let roots = ["/root-a"];
    let mut directory = repository_prelude(&roots, 2);
    directory.extend([
        present("/root-a/pkg", PathNodeKind::Directory, 2),
        present("/root-a/pkg/BUILD.bazel", PathNodeKind::Directory, 2),
        missing("/root-a/pkg/BUILD"),
    ]);
    assert_eq!(
        complete(&boundary(Some(inputs(&roots, &[])), directory, "pkg").await).kind(),
        HostRootPackageBoundaryKind::NoPackage
    );

    for (kind, target) in [
        (PathNodeKind::RegularFile, None),
        (PathNodeKind::SpecialFile, None),
        (PathNodeKind::Symlink, Some("/outside/marker")),
    ] {
        let mut entries = repository_prelude(&roots, 3);
        entries.push(present("/root-a/pkg", PathNodeKind::Directory, 3));
        entries.push(present("/root-a/pkg/BUILD.bazel", kind, 3));
        if let Some(target) = target {
            entries.push(read_link("/root-a/pkg/BUILD.bazel", target));
            entries.push(present("/outside", PathNodeKind::Directory, 3));
            entries.push(present(target, PathNodeKind::RegularFile, 3));
        }
        assert_eq!(
            complete(&boundary(Some(inputs(&roots, &[])), entries, "pkg").await).kind(),
            HostRootPackageBoundaryKind::Package
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn ignore_precedes_deleted_invalid_and_marker_without_lookup_demands() {
    let roots = ["/root-a"];
    let mut entries = repository_prelude(&roots, 1);
    entries.retain(|(demand, _)| {
        demand.path().as_path() != std::path::Path::new("/root-a/.bazelignore")
    });
    entries.extend([
        present("/root-a/.bazelignore", PathNodeKind::RegularFile, 1),
        bytes("/root-a/.bazelignore", b"ignored\n"),
        present("/root-a/ignored/marker", PathNodeKind::Directory, 1),
        present(
            "/root-a/ignored/marker/BUILD.bazel",
            PathNodeKind::RegularFile,
            1,
        ),
    ]);

    for (package, deleted) in [
        ("ignored", &["//ignored"][..]),
        ("ignored/bad:name", &[][..]),
        ("ignored/marker", &[][..]),
    ] {
        let (result, tracker) =
            tracked_boundary(inputs(&roots, deleted), entries.clone(), package).await;
        assert_eq!(
            complete(&result).kind(),
            HostRootPackageBoundaryKind::IgnoredDirectory
        );
        assert_eq!(tracker.lookup_activations.load(Ordering::SeqCst), 0);
        assert_eq!(
            *tracker.boundary_dependencies.lock().unwrap(),
            vec![vec!["host-repository-ignore:\"/workspace\"".to_owned()]]
        );
        assert!(
            tracker
                .activated
                .lock()
                .unwrap()
                .iter()
                .all(|key| !key.contains("/root-a/ignored/marker/BUILD"))
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn invalid_latin_one_scalars_are_distinct_no_package_without_marker_demands() {
    let roots = ["/root-a"];
    let entries = repository_prelude(&roots, 1);
    let first = boundary(Some(inputs(&roots, &[])), entries.clone(), "bad\u{e9}").await;
    let second = boundary(Some(inputs(&roots, &[])), entries, "bad\u{c3}\u{a9}").await;
    assert_eq!(
        complete(&first).kind(),
        HostRootPackageBoundaryKind::NoPackage
    );
    assert_eq!(
        complete(&second).kind(),
        HostRootPackageBoundaryKind::NoPackage
    );
    assert_ne!(
        HostRootPackageBoundaryKey::new(
            path("/workspace"),
            PackagePath::parse("bad\u{e9}").unwrap()
        ),
        HostRootPackageBoundaryKey::new(
            path("/workspace"),
            PackagePath::parse("bad\u{c3}\u{a9}").unwrap()
        ),
    );
}

#[cfg(unix)]
#[tokio::test]
async fn typed_errors_need_and_error_surface_remain_opaque() {
    let roots = ["/root-a"];
    let ignore_failure = vec![
        present("/", PathNodeKind::Directory, 1),
        present("/workspace", PathNodeKind::Directory, 1),
        lstat_error("/workspace/REPO.bazel"),
    ];
    let ignore = boundary(Some(inputs(&roots, &[])), ignore_failure, "pkg").await;
    let PathOutcome::Complete(ignore) = ignore else {
        panic!("ignore error must complete")
    };
    let Err(ignore) = ignore.as_ref() else {
        panic!("expected ignore error")
    };
    assert!(matches!(
        ignore.inner,
        HostRootPackageBoundaryErrorInner::RepositoryIgnore(_)
    ));
    let ignore_outcome = PathOutcome::Complete(Arc::new(Err(ignore.clone())));
    assert!(HostRootPackageBoundaryKey::validity(&ignore_outcome));
    assert!(HostRootPackageBoundaryKey::equality(
        &ignore_outcome,
        &ignore_outcome
    ));
    assert_eq!(format!("{:?}", ignore), "HostRootPackageBoundaryError");
    match &ignore.inner {
        HostRootPackageBoundaryErrorInner::RepositoryIgnore(error) => {
            assert_eq!(ignore.to_string(), error.to_string());
            assert_eq!(
                std::error::Error::source(ignore).is_some(),
                std::error::Error::source(error).is_some()
            );
        }
        HostRootPackageBoundaryErrorInner::PackageLookup(_) => unreachable!(),
    }

    let mut lookup_failure = repository_prelude(&roots, 2);
    lookup_failure.extend([
        present("/root-a/pkg", PathNodeKind::Directory, 2),
        lstat_error("/root-a/pkg/BUILD.bazel"),
    ]);
    let lookup = boundary(Some(inputs(&roots, &[])), lookup_failure, "pkg").await;
    let PathOutcome::Complete(lookup) = lookup else {
        panic!("lookup error must complete")
    };
    let Err(lookup) = lookup.as_ref() else {
        panic!("expected lookup error")
    };
    assert!(matches!(
        lookup.inner,
        HostRootPackageBoundaryErrorInner::PackageLookup(_)
    ));
    let lookup_outcome = PathOutcome::Complete(Arc::new(Err(lookup.clone())));
    assert!(HostRootPackageBoundaryKey::validity(&lookup_outcome));
    assert!(HostRootPackageBoundaryKey::equality(
        &lookup_outcome,
        &lookup_outcome
    ));
    assert_eq!(format!("{:?}", lookup), "HostRootPackageBoundaryError");
    match &lookup.inner {
        HostRootPackageBoundaryErrorInner::PackageLookup(error) => {
            assert_eq!(lookup.to_string(), error.to_string());
            assert_eq!(
                std::error::Error::source(lookup).is_some(),
                std::error::Error::source(error).is_some()
            );
        }
        HostRootPackageBoundaryErrorInner::RepositoryIgnore(_) => unreachable!(),
    }

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, inputs(&roots, &[])).unwrap();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::empty(),
        )])
        .unwrap();
    let mut transaction = updater.commit().await;
    let expected = transaction
        .compute(&HostRepositoryIgnoreKey::new(path("/workspace")))
        .await
        .unwrap();
    let need = transaction
        .compute(&HostRootPackageBoundaryKey::new(
            path("/workspace"),
            PackagePath::parse("pkg").unwrap(),
        ))
        .await
        .unwrap();
    let (PathOutcome::Need(expected), PathOutcome::Need(actual)) = (expected, need.dupe()) else {
        panic!("boundary must pass the ignore Need unchanged");
    };
    assert_eq!(actual, expected);
    assert!(!HostRootPackageBoundaryKey::validity(&need));
    assert!(!HostRootPackageBoundaryKey::equality(&need, &need));
}

#[cfg(unix)]
#[tokio::test]
async fn retained_dice_prunes_marker_changes_and_replays_boundary_transitions() {
    fn script(marker: Option<i64>, ignore: bool, root_b: bool, variant: i64) -> Vec<ScriptEntry> {
        let roots = ["/root-a", "/root-b"];
        let mut entries = repository_prelude(&roots, variant);
        if ignore {
            entries.retain(|(demand, _)| {
                demand.path().as_path() != std::path::Path::new("/root-a/.bazelignore")
            });
            entries.push(present(
                "/root-a/.bazelignore",
                PathNodeKind::RegularFile,
                variant,
            ));
            entries.push(bytes("/root-a/.bazelignore", b"pkg\n"));
        }
        if let Some(marker) = marker {
            if root_b {
                entries.extend([
                    present("/root-a/pkg", PathNodeKind::Directory, variant),
                    missing("/root-a/pkg/BUILD.bazel"),
                    missing("/root-a/pkg/BUILD"),
                    present("/root-b/pkg", PathNodeKind::Directory, variant),
                    present("/root-b/pkg/BUILD.bazel", PathNodeKind::RegularFile, marker),
                ]);
            } else {
                entries.push(present("/root-a/pkg", PathNodeKind::Directory, variant));
                entries.push(present(
                    "/root-a/pkg/BUILD.bazel",
                    PathNodeKind::RegularFile,
                    marker,
                ));
            }
        } else {
            for root in roots {
                entries.extend([
                    present(&format!("{root}/pkg"), PathNodeKind::Directory, variant),
                    missing(&format!("{root}/pkg/BUILD.bazel")),
                    missing(&format!("{root}/pkg/BUILD")),
                ]);
            }
        }
        entries
    }

    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    inject_root_package_policy_inputs(&mut updater, inputs(&["/root-a", "/root-b"], &[])).unwrap();
    let mut transaction = updater.commit().await;
    let boundary =
        HostRootPackageBoundaryKey::new(path("/workspace"), PackagePath::parse("pkg").unwrap());
    let counter = BoundaryCounterKey {
        boundary: boundary.clone(),
        counter: Arc::new(AtomicUsize::new(0)),
    };

    transaction = update_epoch(transaction, &script(None, false, false, 1)).await;
    let absent = transaction.compute(&boundary).await.unwrap();
    assert_eq!(
        complete(&absent).kind(),
        HostRootPackageBoundaryKind::NoPackage
    );
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(1)
    ));

    transaction = update_epoch(transaction, &script(Some(2), false, false, 2)).await;
    let created = transaction.compute(&boundary).await.unwrap();
    assert_eq!(
        complete(&created).selected_package_root(),
        Some(&path("/root-a"))
    );
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(2)
    ));

    transaction = update_epoch(transaction, &script(Some(99), false, false, 99)).await;
    let changed_marker = transaction.compute(&boundary).await.unwrap();
    assert!(HostRootPackageBoundaryKey::equality(
        &created,
        &changed_marker
    ));
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(2)
    ));

    let mut lookup_error = repository_prelude(&["/root-a", "/root-b"], 100);
    lookup_error.extend([
        present("/root-a/pkg", PathNodeKind::Directory, 100),
        lstat_error("/root-a/pkg/BUILD.bazel"),
    ]);
    transaction = update_epoch(transaction, &lookup_error).await;
    let failed = transaction.compute(&boundary).await.unwrap();
    assert!(matches!(failed, PathOutcome::Complete(value) if value.is_err()));
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(3)
    ));

    transaction = update_epoch(transaction, &script(Some(3), false, false, 3)).await;
    let recovered = transaction.compute(&boundary).await.unwrap();
    assert!(HostRootPackageBoundaryKey::equality(&created, &recovered));
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(4)
    ));

    transaction = update_policy(transaction, inputs(&["/root-a", "/root-b"], &["//pkg"])).await;
    let policy_deleted = transaction.compute(&boundary).await.unwrap();
    assert_eq!(
        complete(&policy_deleted).kind(),
        HostRootPackageBoundaryKind::DeletedPackage
    );
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(5)
    ));

    transaction = update_policy(transaction, inputs(&["/root-a", "/root-b"], &[])).await;
    let policy_restored = transaction.compute(&boundary).await.unwrap();
    assert!(HostRootPackageBoundaryKey::equality(
        &created,
        &policy_restored
    ));
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(6)
    ));

    transaction = update_epoch(transaction, &script(None, false, false, 3)).await;
    let no_marker = transaction.compute(&boundary).await.unwrap();
    assert!(HostRootPackageBoundaryKey::equality(&absent, &no_marker));
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(7)
    ));

    transaction = update_epoch(transaction, &script(Some(4), true, false, 4)).await;
    let ignored = transaction.compute(&boundary).await.unwrap();
    assert_eq!(
        complete(&ignored).kind(),
        HostRootPackageBoundaryKind::IgnoredDirectory
    );
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(8)
    ));

    transaction = update_epoch(transaction, &script(Some(5), false, true, 5)).await;
    let root_b = transaction.compute(&boundary).await.unwrap();
    assert_eq!(
        complete(&root_b).selected_package_root(),
        Some(&path("/root-b"))
    );
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(9)
    ));

    transaction = update_epoch(transaction, &script(Some(6), false, false, 6)).await;
    let restored = transaction.compute(&boundary).await.unwrap();
    assert!(HostRootPackageBoundaryKey::equality(&created, &restored));
    assert!(matches!(
        transaction.compute(&counter).await.unwrap(),
        PathOutcome::Complete(10)
    ));
}
