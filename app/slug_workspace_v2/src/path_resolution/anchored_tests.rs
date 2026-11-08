use dice::DetectCycles;
use dice::Dice;

use super::*;
use crate::PathObservationEpochKey;
use crate::PathObservationInstanceId;

fn path(value: &str) -> NormalizedAbsolutePath {
    NormalizedAbsolutePath::new(PathBuf::from(value)).unwrap()
}

fn anchor(instance: u64, root: &str) -> MaterializationPathAnchor {
    MaterializationPathAnchor::new(
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance)),
        path(root),
    )
    .unwrap()
}

type Entry = (PathObservationDemand, PathObservationResult);

fn lstat(anchor: &MaterializationPathAnchor, value: &str, kind: Option<PathNodeKind>) -> Entry {
    (
        PathObservationDemand::new(
            anchor.namespace(),
            path(value),
            PathObservationOperation::Lstat,
        ),
        PathObservationResult::Lstat(match kind {
            Some(kind) => PathOperationResult::Present(PathLstat::new(kind, 1, 2, 3, 4, 0o755)),
            None => PathOperationResult::Missing,
        }),
    )
}

fn link(anchor: &MaterializationPathAnchor, value: &str, target: &str) -> Entry {
    (
        PathObservationDemand::new(
            anchor.namespace(),
            path(value),
            PathObservationOperation::ReadLink,
        ),
        PathObservationResult::ReadLink(PathOperationResult::Present(Arc::new(PathBuf::from(
            target,
        )))),
    )
}

// Every prefix must ask for exactly the next actual observation. In particular,
// no scripted ancestor can accidentally stand in for the selected physical root.
async fn resolve(
    anchor: MaterializationPathAnchor,
    requested: &str,
    script: Vec<Entry>,
) -> ObservedAnchoredResolvedPath {
    let key = AnchoredResolvedPathObservationKey::new(anchor.dupe(), requested).unwrap();
    let plain = AnchoredResolvedPathKey::new(anchor, requested).unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    for prefix in 0..=script.len() {
        let epoch = PathObservationEpoch::new(script[..prefix].iter().cloned()).unwrap();
        let mut updater = tx.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
            .unwrap();
        tx = updater.commit().await;
        let value = tx.compute(&key).await.unwrap();
        let operational = tx.compute(&plain).await.unwrap();
        if prefix < script.len() {
            let PathOutcome::Need(need) = &value else {
                panic!("premature completion at {prefix}")
            };
            assert_eq!(need.demands(), &[script[prefix].0.dupe()]);
            assert!(!AnchoredResolvedPathObservationKey::validity(&value));
            assert!(!AnchoredResolvedPathObservationKey::equality(
                &value, &value
            ));
            let PathOutcome::Need(plain_need) = operational else {
                panic!("premature operational completion")
            };
            assert_eq!(plain_need.demands(), need.demands());
        } else {
            let PathOutcome::Complete(Ok(observed)) = value else {
                panic!("full epoch did not complete")
            };
            assert_eq!(observed.observations().observations().len(), script.len());
            for (demand, expected) in epoch.observations() {
                // Growing injected epochs recreate equal fixture Arcs. DICE may
                // retain an earlier equal PathObservationKey value; that actual
                // dependency, not the newest injection allocation, is authority.
                let PathOutcome::Complete(authoritative) = tx
                    .compute(&PathObservationKey::new(demand.dupe()))
                    .await
                    .unwrap()
                else {
                    panic!("completed resolver lost its observation dependency")
                };
                assert_eq!(authoritative.as_ref(), expected.as_ref());
                assert!(Arc::ptr_eq(
                    &authoritative,
                    observed.observations().observations().get(demand).unwrap()
                ));
            }
            let PathOutcome::Complete(result) = operational else {
                panic!("operational result missing")
            };
            assert_eq!(&result, observed.result());
            if let Ok(resolved) = observed.result() {
                assert_eq!(resolved.anchor(), Some(plain.anchor()));
            }
            return observed;
        }
    }
    unreachable!()
}

#[test]
fn anchored_resolution_constructor_and_structural_identity() {
    use std::hash::Hash;
    use std::hash::Hasher;
    fn hash(value: &impl Hash) -> u64 {
        let mut state = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut state);
        state.finish()
    }
    for namespace in [
        PathObservationNamespace::Host,
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(0)),
    ] {
        assert_eq!(
            MaterializationPathAnchor::new(namespace, path("/repo")),
            Err(MaterializationPathAnchorError::NotNonzeroMaterialization)
        );
    }
    let a = anchor(1, "/repo");
    for value in ["/", "/other", "/repository/x", "/repo/../repo/file"] {
        assert_eq!(
            AnchoredResolvedPathKey::new(a.dupe(), value),
            Err(MaterializationPathAnchorError::EscapesRoot)
        );
    }
    assert_eq!(
        AnchoredResolvedPathKey::new(a.dupe(), "relative"),
        Err(MaterializationPathAnchorError::RelativePath)
    );
    let key = AnchoredResolvedPathKey::new(a.dupe(), "/repo/sub/file").unwrap();
    let equal = AnchoredResolvedPathKey::new(a, "/repo/sub/./file").unwrap();
    assert_eq!(key, equal);
    assert_eq!(hash(&key), hash(&equal));
    for other in [anchor(2, "/repo"), anchor(1, "/repo/sub")] {
        let different = AnchoredResolvedPathKey::new(other, "/repo/sub/file").unwrap();
        assert_ne!(key, different);
        assert_ne!(hash(&key), hash(&different));
    }
}

#[tokio::test]
async fn anchored_resolution_demands_real_root_without_host_ancestors() {
    let a = anchor(1, "/unobserved/parent/repo");
    let observed = resolve(
        a.dupe(),
        "/unobserved/parent/repo/file",
        vec![
            lstat(&a, "/unobserved/parent/repo", Some(PathNodeKind::Directory)),
            lstat(
                &a,
                "/unobserved/parent/repo/file",
                Some(PathNodeKind::RegularFile),
            ),
        ],
    )
    .await;
    assert_eq!(
        observed.result().as_ref().unwrap().real_path(),
        &path("/unobserved/parent/repo/file")
    );
    assert!(
        observed
            .observations()
            .observations()
            .keys()
            .all(|demand| demand.path().as_path().starts_with(a.root().as_path()))
    );
    let missing = resolve(
        a.dupe(),
        "/unobserved/parent/repo/file",
        vec![lstat(&a, "/unobserved/parent/repo", None)],
    )
    .await;
    assert_eq!(
        missing.result().as_ref().unwrap().state(),
        ResolvedPathState::Missing
    );
    let wrong = resolve(
        a.dupe(),
        "/unobserved/parent/repo",
        vec![lstat(
            &a,
            "/unobserved/parent/repo",
            Some(PathNodeKind::Symlink),
        )],
    )
    .await;
    assert!(matches!(
        wrong.result(),
        Err(AnchoredPathResolutionError::Resolution(
            PathResolutionError::Observation {
                error: PathObservationError::WrongKind {
                    expected: PathNodeKind::Directory,
                    actual: PathNodeKind::Symlink
                },
                ..
            }
        ))
    ));
}

#[tokio::test]
async fn anchored_resolution_follows_nested_relative_and_absolute_internal_links() {
    let a = anchor(3, "/repo");
    let observed = resolve(
        a.dupe(),
        "/repo/alias/file",
        vec![
            lstat(&a, "/repo", Some(PathNodeKind::Directory)),
            lstat(&a, "/repo/alias", Some(PathNodeKind::Symlink)),
            link(&a, "/repo/alias", "dir"),
            lstat(&a, "/repo/dir", Some(PathNodeKind::Directory)),
            lstat(&a, "/repo/dir/file", Some(PathNodeKind::Symlink)),
            link(&a, "/repo/dir/file", "/repo/real"),
            lstat(&a, "/repo/real", Some(PathNodeKind::RegularFile)),
        ],
    )
    .await;
    let result = observed.result().as_ref().unwrap();
    assert_eq!(result.real_path(), &path("/repo/real"));
    assert_eq!(result.symlinks().len(), 2);
    let up = resolve(
        a.dupe(),
        "/repo/dir/link",
        vec![
            lstat(&a, "/repo", Some(PathNodeKind::Directory)),
            lstat(&a, "/repo/dir", Some(PathNodeKind::Directory)),
            lstat(&a, "/repo/dir/link", Some(PathNodeKind::Symlink)),
            link(&a, "/repo/dir/link", "../file"),
            lstat(&a, "/repo/file", Some(PathNodeKind::RegularFile)),
        ],
    )
    .await;
    assert_eq!(
        up.result().as_ref().unwrap().real_path(),
        &path("/repo/file")
    );
}

#[tokio::test]
async fn anchored_resolution_refuses_absolute_relative_and_reentering_link_escapes() {
    let a = anchor(4, "/repo");
    for target in [
        "/outside",
        "../outside",
        "../repo/inside",
        "/repo/../repo/inside",
    ] {
        let observed = resolve(
            a.dupe(),
            "/repo/link",
            vec![
                lstat(&a, "/repo", Some(PathNodeKind::Directory)),
                lstat(&a, "/repo/link", Some(PathNodeKind::Symlink)),
                link(&a, "/repo/link", target),
            ],
        )
        .await;
        assert!(
            matches!(observed.result(), Err(AnchoredPathResolutionError::EscapesAnchor { anchor, requested_path }) if anchor == &a && requested_path == &path("/repo/link"))
        );
    }
}

#[tokio::test]
async fn anchored_resolution_preserves_cycle_and_expansion_frontiers() {
    let a = anchor(5, "/repo");
    for (target, expansion) in [("link", false), ("link/child", true)] {
        let observed = resolve(
            a.dupe(),
            "/repo/link",
            vec![
                lstat(&a, "/repo", Some(PathNodeKind::Directory)),
                lstat(&a, "/repo/link", Some(PathNodeKind::Symlink)),
                link(&a, "/repo/link", target),
            ],
        )
        .await;
        assert!(match observed.result() {
            Err(AnchoredPathResolutionError::Resolution(PathResolutionError::Cycle { .. })) =>
                !expansion,
            Err(AnchoredPathResolutionError::Resolution(
                PathResolutionError::InfiniteExpansion { .. },
            )) => expansion,
            _ => false,
        });
    }
}

#[tokio::test]
async fn anchored_resolution_rejects_foreign_namespace_and_preserves_host_resolver() {
    let a = anchor(6, "/repo");
    let foreign = anchor(7, "/repo");
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new([lstat(&foreign, "/repo", Some(PathNodeKind::Directory))])
                .unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let PathOutcome::Need(need) = tx
        .compute(&AnchoredResolvedPathKey::new(a.dupe(), "/repo").unwrap())
        .await
        .unwrap()
    else {
        panic!("foreign epoch granted authority")
    };
    assert_eq!(need.demands(), &[lstat(&a, "/repo", None).0]);
    let PathOutcome::Need(host) = tx
        .compute(&ResolvedPathKey::new(
            PathObservationNamespace::Host,
            path("/repo/file"),
        ))
        .await
        .unwrap()
    else {
        panic!("Host root must still be observed")
    };
    assert_eq!(
        host.demands(),
        &[PathObservationDemand::new(
            PathObservationNamespace::Host,
            path("/"),
            PathObservationOperation::Lstat
        )]
    );
}

#[tokio::test]
async fn anchored_resolution_observed_epoch_tracks_complete_root_changes_and_restoration() {
    let a = anchor(8, "/repo");
    let key = AnchoredResolvedPathObservationKey::new(a.dupe(), "/repo").unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    let original = lstat(&a, "/repo", Some(PathNodeKind::Directory));
    let changed = (
        original.0.dupe(),
        PathObservationResult::Lstat(PathOperationResult::Present(PathLstat::new(
            PathNodeKind::Directory,
            10,
            20,
            30,
            40,
            0o700,
        ))),
    );
    let mut values = Vec::new();
    for row in [original.clone(), changed, original] {
        let epoch = PathObservationEpoch::new([row]).unwrap();
        let mut updater = tx.into_updater();
        updater
            .changed_to(vec![(PathObservationEpochKey, epoch.dupe())])
            .unwrap();
        tx = updater.commit().await;
        let value = tx.compute(&key).await.unwrap();
        let PathOutcome::Complete(Ok(observed)) = &value else {
            panic!("complete root observation missing")
        };
        assert_eq!(observed.observations(), &epoch);
        values.push(value);
    }
    assert!(!AnchoredResolvedPathObservationKey::equality(
        &values[0], &values[1]
    ));
    assert!(AnchoredResolvedPathObservationKey::equality(
        &values[0], &values[2]
    ));
}
