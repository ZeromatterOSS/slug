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

fn listing(anchor: &MaterializationPathAnchor, value: &str) -> Entry {
    (
        PathObservationDemand::new(
            anchor.namespace(),
            path(value),
            PathObservationOperation::DirectoryEntries,
        ),
        PathObservationResult::DirectoryEntries(PathOperationResult::Present(
            PathDirectoryEntries::new([crate::PathDirectoryEntry::new(
                crate::PathDirectoryName::new("child").unwrap(),
                crate::PathDirectoryEntryKind::File,
            )]),
        )),
    )
}

async fn anchored_listing(
    anchor: MaterializationPathAnchor,
    requested: &str,
    script: Vec<Entry>,
) -> ObservedPathDirectoryListing {
    let key = PathDirectoryListingObservationKey::new_anchored(anchor.dupe(), requested).unwrap();
    let plain = PathDirectoryListingKey::new_anchored(anchor.dupe(), requested).unwrap();
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut tx = dice.updater().commit().await;
    for prefix in 0..=script.len() {
        let mut updater = tx.into_updater();
        updater
            .changed_to(vec![(
                PathObservationEpochKey,
                PathObservationEpoch::new(script[..prefix].iter().cloned()).unwrap(),
            )])
            .unwrap();
        tx = updater.commit().await;
        let value = tx.compute(&key).await.unwrap();
        let operational = tx.compute(&plain).await.unwrap();
        if prefix < script.len() {
            let PathOutcome::Need(need) = &value else {
                panic!("premature listing at {prefix}")
            };
            assert_eq!(need.demands(), &[script[prefix].0.dupe()]);
            assert!(
                need.demands()
                    .iter()
                    .all(|demand| demand.namespace() == anchor.namespace()
                        && demand.path().as_path().starts_with(anchor.root().as_path()))
            );
            assert!(!PathDirectoryListingObservationKey::validity(&value));
            assert!(!PathDirectoryListingObservationKey::equality(
                &value, &value
            ));
            let PathOutcome::Need(plain_need) = operational else {
                panic!("premature plain listing")
            };
            assert_eq!(plain_need.demands(), need.demands());
        } else {
            let PathOutcome::Complete(Ok(observed)) = value else {
                panic!("listing did not complete")
            };
            let PathOutcome::Complete(plain_result) = operational else {
                panic!("plain listing incomplete")
            };
            assert_eq!(&plain_result, observed.result());
            assert_eq!(observed.observations().observations().len(), script.len());
            for (demand, _) in &script {
                let PathOutcome::Complete(authoritative) = tx
                    .compute(&PathObservationKey::new(demand.dupe()))
                    .await
                    .unwrap()
                else {
                    panic!("dependency lost")
                };
                assert!(Arc::ptr_eq(
                    &authoritative,
                    observed.observations().observations().get(demand).unwrap()
                ));
            }
            return observed;
        }
    }
    unreachable!()
}

#[tokio::test]
async fn anchored_directory_listing_stops_at_root_and_follows_only_internal_links() {
    let a = anchor(101, "/absent/parent/repo");
    let value = anchored_listing(
        a.dupe(),
        "/absent/parent/repo/alias",
        vec![
            lstat(&a, "/absent/parent/repo", Some(PathNodeKind::Directory)),
            lstat(&a, "/absent/parent/repo/alias", Some(PathNodeKind::Symlink)),
            link(&a, "/absent/parent/repo/alias", "dir"),
            lstat(&a, "/absent/parent/repo/dir", Some(PathNodeKind::Directory)),
            listing(&a, "/absent/parent/repo/dir"),
        ],
    )
    .await;
    assert!(
        matches!(value.result(), Ok(PathDirectoryListing::Present(entries)) if entries.entries().len()==1)
    );
}

#[tokio::test]
async fn anchored_directory_listing_refuses_escape_without_outside_demand() {
    let a = anchor(102, "/absent/parent/repo");
    let value = anchored_listing(
        a.dupe(),
        "/absent/parent/repo/link",
        vec![
            lstat(&a, "/absent/parent/repo", Some(PathNodeKind::Directory)),
            lstat(&a, "/absent/parent/repo/link", Some(PathNodeKind::Symlink)),
            link(&a, "/absent/parent/repo/link", "../outside"),
        ],
    )
    .await;
    let expected = PathDirectoryListingError::EscapesAnchor {
        logical_path: path("/absent/parent/repo/link"),
        anchor: a.dupe(),
    };
    assert_eq!(value.result(), &Err(expected.clone()));
    assert_ne!(
        expected,
        PathDirectoryListingError::EscapesAnchor {
            logical_path: path("/absent/parent/repo/link"),
            anchor: anchor(103, "/absent/parent/repo"),
        }
    );
}

#[tokio::test]
async fn anchored_directory_listing_preserves_missing_and_wrong_kind() {
    let a = anchor(104, "/absent/repo");
    let missing = anchored_listing(
        a.dupe(),
        "/absent/repo",
        vec![lstat(&a, "/absent/repo", None)],
    )
    .await;
    assert_eq!(missing.result(), &Ok(PathDirectoryListing::Missing));
    let wrong = anchored_listing(
        a.dupe(),
        "/absent/repo",
        vec![lstat(&a, "/absent/repo", Some(PathNodeKind::RegularFile))],
    )
    .await;
    assert_eq!(
        wrong.result(),
        &Err(PathDirectoryListingError::Observation {
            logical_path: path("/absent/repo"),
            operation: PathObservationOperation::Lstat,
            error: PathObservationError::WrongKind {
                expected: PathNodeKind::Directory,
                actual: PathNodeKind::RegularFile,
            },
        })
    );
    let descendant = anchored_listing(
        a.dupe(),
        "/absent/repo/file",
        vec![
            lstat(&a, "/absent/repo", Some(PathNodeKind::Directory)),
            lstat(&a, "/absent/repo/file", Some(PathNodeKind::RegularFile)),
        ],
    )
    .await;
    assert_eq!(
        descendant.result(),
        &Err(PathDirectoryListingError::WrongKind {
            logical_path: path("/absent/repo/file"),
            expected: PathNodeKind::Directory,
            actual: PathNodeKind::RegularFile,
        })
    );
}

#[test]
fn anchored_directory_listing_identity_and_raw_membership_are_explicit() {
    let a = anchor(105, "/repo");
    let b = anchor(106, "/repo");
    let anchored = PathDirectoryListingKey::new_anchored(a.dupe(), "/repo/dir").unwrap();
    assert_ne!(
        anchored,
        PathDirectoryListingKey::new_anchored(b, "/repo/dir").unwrap()
    );
    assert_ne!(
        anchored,
        PathDirectoryListingKey::new(a.namespace(), path("/repo/dir"))
    );
    for outside in ["/repo-other", "/repo/../repo/dir", "relative"] {
        assert!(PathDirectoryListingKey::new_anchored(a.dupe(), outside).is_err());
        assert!(PathDirectoryListingObservationKey::new_anchored(a.dupe(), outside).is_err());
    }
}

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

fn source_context() -> (MaterializationPathAnchor, MaterializationPathAnchor) {
    let source = anchor(302, "/source");
    let logical = anchor(301, "/general")
        .with_source_anchors(vec![
            MaterializationTerminalAnchor::new(source.namespace(), source.root().dupe()).unwrap(),
        ])
        .unwrap();
    (logical, source)
}

#[test]
fn source_anchor_context_rejects_aliasing_overlap_and_extra_authority() {
    let logical = anchor(301, "/general");
    let leaf = |instance: u64, root: &str| {
        MaterializationTerminalAnchor::new(
            PathObservationNamespace::Materialization(PathObservationInstanceId::new(instance)),
            path(root),
        )
        .unwrap()
    };
    for leaves in [
        vec![leaf(301, "/source")],
        vec![leaf(302, "/general/inside")],
        vec![leaf(302, "/")],
        vec![leaf(302, "/source"), leaf(303, "/source/inside")],
        vec![leaf(302, "/source"), leaf(302, "/other")],
        vec![leaf(302, "/source"), leaf(303, "/source")],
    ] {
        assert!(logical.dupe().with_source_anchors(leaves).is_err());
    }
    let too_many = (0..129).map(|i| leaf(400 + i, &format!("/s{i}"))).collect();
    assert!(logical.dupe().with_source_anchors(too_many).is_err());
    let a = logical
        .dupe()
        .with_source_anchors(vec![leaf(303, "/z"), leaf(302, "/a")])
        .unwrap();
    let b = logical
        .with_source_anchors(vec![leaf(302, "/a"), leaf(303, "/z")])
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(a.source_anchors()[0].root(), &path("/a"));
    assert!(a.with_source_anchors(Vec::new()).is_err());
}

#[tokio::test]
async fn source_link_resolution_keeps_logical_owner_and_terminal_content_namespace() {
    let (logical, source) = source_context();
    let script = vec![
        lstat(&logical, "/general", Some(PathNodeKind::Directory)),
        lstat(&logical, "/general/link", Some(PathNodeKind::Symlink)),
        link(&logical, "/general/link", "/source/file"),
        lstat(&source, "/source", Some(PathNodeKind::Directory)),
        lstat(&source, "/source/file", Some(PathNodeKind::RegularFile)),
    ];
    let value = resolve(logical.dupe(), "/general/link", script.clone()).await;
    let result = value.result().as_ref().unwrap();
    assert_eq!(result.namespace(), logical.namespace());
    assert_eq!(result.anchor(), Some(&logical));
    assert_eq!(result.requested_path(), &path("/general/link"));
    assert_eq!(result.real_path(), &path("/source/file"));
    assert_eq!(result.content_origin().namespace(), source.namespace());
    assert_eq!(result.content_origin().anchor(), Some(&source));
    assert_eq!(result.symlinks()[0].namespace(), logical.namespace());
    let key = AnchoredResolvedPathKey::new(logical, "/general/link").unwrap();
    let mut demands = Vec::new();
    let synchronous = key
        .resolve_sync::<std::convert::Infallible>(|demand| {
            demands.push(demand.dupe());
            Ok(script
                .iter()
                .find(|(d, _)| d == demand)
                .expect("only admitted root facts")
                .1
                .clone())
        })
        .unwrap()
        .unwrap();
    assert_eq!(&synchronous, result);
    assert_eq!(
        demands,
        script.iter().map(|(d, _)| d.dupe()).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn source_directory_links_keep_remaining_components_and_missing_origin() {
    let (logical, source) = source_context();
    let prefix = vec![
        lstat(&logical, "/general", Some(PathNodeKind::Directory)),
        lstat(&logical, "/general/link", Some(PathNodeKind::Symlink)),
        link(&logical, "/general/link", "/source/dir"),
        lstat(&source, "/source", Some(PathNodeKind::Directory)),
        lstat(&source, "/source/dir", Some(PathNodeKind::Directory)),
    ];
    for kind in [Some(PathNodeKind::RegularFile), None] {
        let mut script = prefix.clone();
        script.push(lstat(&source, "/source/dir/child", kind));
        let value = resolve(logical.dupe(), "/general/link/child", script).await;
        let result = value.result().as_ref().unwrap();
        assert_eq!(result.real_path(), &path("/source/dir/child"));
        assert_eq!(result.namespace(), logical.namespace());
        assert_eq!(result.content_origin().anchor(), Some(&source));
        assert_eq!(
            matches!(result.state(), ResolvedPathState::Missing),
            kind.is_none()
        );
    }
}

#[tokio::test]
async fn source_resolution_refuses_undeclared_roots_relative_escape_and_source_reentry() {
    let (logical, source) = source_context();
    for target in [
        "/undeclared/file",
        "../source/file",
        "/source/absent/../file",
        "/source//file",
    ] {
        let value = resolve(
            logical.dupe(),
            "/general/link",
            vec![
                lstat(&logical, "/general", Some(PathNodeKind::Directory)),
                lstat(&logical, "/general/link", Some(PathNodeKind::Symlink)),
                link(&logical, "/general/link", target),
            ],
        )
        .await;
        assert!(
            matches!(
                value.result(),
                Err(AnchoredPathResolutionError::EscapesAnchor { .. })
            ),
            "{target}"
        );
    }
    let value = resolve(
        logical.dupe(),
        "/general/link",
        vec![
            lstat(&logical, "/general", Some(PathNodeKind::Directory)),
            lstat(&logical, "/general/link", Some(PathNodeKind::Symlink)),
            link(&logical, "/general/link", "/source/back"),
            lstat(&source, "/source", Some(PathNodeKind::Directory)),
            lstat(&source, "/source/back", Some(PathNodeKind::Symlink)),
            link(&source, "/source/back", "/general/link"),
        ],
    )
    .await;
    assert!(matches!(
        value.result(),
        Err(AnchoredPathResolutionError::EscapesAnchor { .. })
    ));
}

#[tokio::test]
async fn source_content_digest_and_listing_preserve_the_complete_mixed_frontier() {
    use crate::AnchoredPathFileDigestObservationKey;
    use crate::FileContentDigest;
    use crate::PathFileDigest;
    let (logical, source) = source_context();
    let digest = FileContentDigest::new([8; 32], 1).unwrap();
    let mut script = vec![
        lstat(&logical, "/general", Some(PathNodeKind::Directory)),
        lstat(&logical, "/general/link", Some(PathNodeKind::Symlink)),
        link(&logical, "/general/link", "/source/file"),
        lstat(&source, "/source", Some(PathNodeKind::Directory)),
        lstat(&source, "/source/file", Some(PathNodeKind::RegularFile)),
    ];
    script.push((
        PathObservationDemand::new(
            source.namespace(),
            path("/source/file"),
            PathObservationOperation::FileDigest,
        ),
        PathObservationResult::FileDigest(PathOperationResult::Present(digest)),
    ));
    let dice = Dice::builder().build(DetectCycles::Enabled);
    let mut updater = dice.updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(script.clone()).unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let key = AnchoredPathFileDigestObservationKey::new(logical.dupe(), "/general/link").unwrap();
    let PathOutcome::Complete(Ok(value)) = tx.compute(&key).await.unwrap() else {
        panic!("terminal digest must use Source namespace")
    };
    assert_eq!(value.result(), &Ok(PathFileDigest::Present(digest)));
    assert_eq!(value.observations().observations().len(), script.len());
    script.truncate(3);
    script[2] = link(&logical, "/general/link", "/source/dir");
    script.extend([
        lstat(&source, "/source", Some(PathNodeKind::Directory)),
        lstat(&source, "/source/dir", Some(PathNodeKind::Directory)),
        listing(&source, "/source/dir"),
    ]);
    let mut updater = tx.into_updater();
    updater
        .changed_to(vec![(
            PathObservationEpochKey,
            PathObservationEpoch::new(script.clone()).unwrap(),
        )])
        .unwrap();
    let mut tx = updater.commit().await;
    let key = PathDirectoryListingObservationKey::new_anchored(logical, "/general/link").unwrap();
    let PathOutcome::Complete(Ok(value)) = tx.compute(&key).await.unwrap() else {
        panic!("terminal listing must use Source namespace")
    };
    assert!(matches!(
        value.result(),
        Ok(PathDirectoryListing::Present(_))
    ));
    assert_eq!(value.observations().observations().len(), script.len());
}

#[test]
fn guarded_sync_simple_path_uses_actual_small_work_peak_and_matches_unguarded() {
    let anchor = anchor(41, "/repo");
    let key = AnchoredResolvedPathKey::new(anchor, "/repo/file").unwrap();
    let observe = |demand: &PathObservationDemand| -> Result<PathObservationResult, ()> {
        assert_eq!(demand.operation(), PathObservationOperation::Lstat);
        Ok(PathObservationResult::Lstat(PathOperationResult::Present(
            PathLstat::new(
                if demand.path().as_path() == Path::new("/repo") {
                    PathNodeKind::Directory
                } else {
                    PathNodeKind::RegularFile
                },
                1,
                0,
                0,
                0,
                0o644,
            ),
        )))
    };
    let mut peak = 0;
    let mut checkpoints = 0;
    let guarded = key
        .resolve_sync_with_work(observe, |bytes| {
            assert!(bytes >= peak);
            peak = bytes;
            checkpoints += 1;
            if bytes <= 8 * 1024 * 1024 {
                Ok(())
            } else {
                Err(())
            }
        })
        .unwrap()
        .unwrap();
    assert!(peak > 0 && peak < 8 * 1024 * 1024);
    assert!(checkpoints > 2);
    assert_eq!(guarded, key.resolve_sync(observe).unwrap().unwrap());
}

#[test]
fn guarded_sync_parent_growth_refuses_or_cancels_before_first_fact() {
    let key = AnchoredResolvedPathKey::new(
        anchor(41, "/repo"),
        format!("/repo{}", "/child".repeat(512)),
    )
    .unwrap();
    let facts = std::cell::Cell::new(0);
    let mut checkpoints = 0;
    let refused = key.resolve_sync_with_work(
        |_| -> Result<PathObservationResult, &str> {
            facts.set(facts.get() + 1);
            panic!("guard must stop before first fact")
        },
        |_| {
            checkpoints += 1;
            if checkpoints == 3 {
                Err("scope canceled")
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(refused, Err("scope canceled"));
    assert_eq!(checkpoints, 3);
    assert_eq!(facts.get(), 0);
    let mut peak = 0;
    let refused = key.resolve_sync_with_work(
        |_| -> Result<PathObservationResult, &str> {
            facts.set(facts.get() + 1);
            panic!("frame budget must stop before first fact")
        },
        |bytes| {
            peak = peak.max(bytes);
            if bytes > 64 * 1024 {
                Err("working credit")
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(refused, Err("working credit"));
    assert!(peak > 64 * 1024);
    assert_eq!(facts.get(), 0);
}

#[test]
fn guarded_sync_preserves_cycle_expansion_and_mixed_source_demand_order() {
    fn compare(key: AnchoredResolvedPathKey, script: Vec<Entry>) {
        let run = |guarded: bool| {
            let mut demands = Vec::new();
            let mut index = 0;
            let mut peak = 0;
            let mut observe =
                |demand: &PathObservationDemand| -> Result<PathObservationResult, ()> {
                    let (expected, result) =
                        script.get(index).expect("unexpected extra resolver demand");
                    assert_eq!(demand, expected, "demand {index}");
                    index += 1;
                    demands.push(demand.dupe());
                    Ok(result.clone())
                };
            let result = if guarded {
                key.resolve_sync_with_work(&mut observe, |bytes| {
                    assert!(bytes >= peak);
                    peak = bytes;
                    Ok(())
                })
            } else {
                key.resolve_sync(&mut observe)
            }
            .unwrap();
            assert_eq!(index, script.len());
            if guarded {
                assert!(peak > 0);
            }
            (result, demands)
        };
        let plain = run(false);
        let guarded = run(true);
        assert_eq!(
            guarded, plain,
            "complete result includes literal hops, content origin and error chains"
        );
    }
    let own = anchor(71, "/repo");
    for target in ["link", "link/child"] {
        compare(
            AnchoredResolvedPathKey::new(own.dupe(), "/repo/link").unwrap(),
            vec![
                lstat(&own, "/repo", Some(PathNodeKind::Directory)),
                lstat(&own, "/repo/link", Some(PathNodeKind::Symlink)),
                link(&own, "/repo/link", target),
            ],
        );
    }
    let (logical, source) = source_context();
    for kind in [Some(PathNodeKind::RegularFile), None] {
        compare(
            AnchoredResolvedPathKey::new(logical.dupe(), "/general/link/child").unwrap(),
            vec![
                lstat(&logical, "/general", Some(PathNodeKind::Directory)),
                lstat(&logical, "/general/link", Some(PathNodeKind::Symlink)),
                link(&logical, "/general/link", "/source/dir"),
                lstat(&source, "/source", Some(PathNodeKind::Directory)),
                lstat(&source, "/source/dir", Some(PathNodeKind::Directory)),
                lstat(&source, "/source/dir/child", kind),
            ],
        );
    }
}
