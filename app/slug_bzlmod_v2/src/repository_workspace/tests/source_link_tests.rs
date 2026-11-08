use sha2::Digest;
use sha2::Sha256;
use slug_workspace_v2::FileContentDigest;
use slug_workspace_v2::ResolvedPathState;

use super::*;
use crate::RepositorySourceSnapshotEntry as SourceEntry;
use crate::RepositorySourceSnapshotNode as SourceNode;
use crate::RepositorySourceSnapshotRequest;
use crate::RepositorySourceSnapshotReservation;
use crate::RepositorySourceSnapshotState;

fn source(instance: u64, lease: u64) -> RepositoryWorkspaceReadView {
    let route = Arc::new(crate::HostCanonicalRepositoryRoute::builtin(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        crate::HostBuiltinBazelToolsRepositoryMapping::testing(),
    ));
    let input = crate::host_canonical_repository_source_input(route, None).unwrap();
    let request = Arc::new(RepositorySourceSnapshotRequest::from_canonical(&input, lease).unwrap());
    let reservation = Arc::new(
        RepositorySourceSnapshotReservation::new(
            request,
            NormalizedAbsolutePath::new("/__slug_repositories__/source-builtin").unwrap(),
            PathObservationInstanceId::new(instance),
        )
        .unwrap(),
    );
    let entry = |p: &str, node| {
        SourceEntry::new(
            p.into(),
            if matches!(&node, SourceNode::Directory) {
                0o755
            } else {
                0o644
            },
            node,
        )
        .unwrap()
    };
    let digest = FileContentDigest::new(Sha256::digest(b"data").into(), 4).unwrap();
    RepositoryWorkspaceReadView::from_source(Arc::new(
        RepositorySourceSnapshotState::new(
            reservation,
            vec![
                entry("", SourceNode::Directory),
                entry("dir", SourceNode::Directory),
                entry("dir/body", SourceNode::File(digest)),
                SourceEntry::new(
                    "internal".into(),
                    0o777,
                    SourceNode::Link("dir/body".into()),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    ))
    .unwrap()
}
fn link(path: &str, target: &str) -> RepositoryWorkspaceEntry {
    RepositoryWorkspaceEntry::new(
        path.into(),
        0o777,
        RepositoryWorkspaceNode::Link(target.into()),
    )
    .unwrap()
}
fn add_link(
    t: &RepositoryWorkspaceTranscript,
    path: &str,
    target: &str,
    source: Option<RepositoryWorkspaceReadView>,
) -> RepositoryWorkspaceTranscript {
    let req = t
        .request(RepositoryWorkspaceOperation::Symlink {
            path: path.into(),
            target: NormalizedAbsolutePath::new(target).unwrap(),
            source,
        })
        .unwrap();
    let mut entries = t.current().entries().to_vec();
    entries.push(link(path, target));
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    t.complete(
        req,
        RepositoryWorkspaceResult::Unit,
        t.current().mode(),
        entries,
    )
    .unwrap()
}
#[test]
fn source_links_resolve_directory_internal_and_dangling_without_changing_logical_owner() {
    let t = add_link(
        &transcript(),
        "dir",
        "/__slug_repositories__/source-builtin/dir",
        Some(source(21, 7)),
    );
    let t = add_link(
        &t,
        "gone",
        "/__slug_repositories__/source-builtin/missing",
        Some(source(21, 7)),
    );
    let t = add_link(
        &t,
        "via",
        "/__slug_repositories__/source-builtin/internal",
        Some(source(21, 7)),
    );
    assert_eq!(t.current().source_bindings().len(), 1);
    let resolved = t.current().resolve_path("dir/body").unwrap();
    assert_eq!(
        resolved.namespace(),
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(11))
    );
    assert_eq!(
        resolved.content_origin().namespace(),
        PathObservationNamespace::Materialization(PathObservationInstanceId::new(21))
    );
    assert_eq!(
        resolved.real_path().as_path(),
        std::path::Path::new("/__slug_repositories__/source-builtin/dir/body")
    );
    assert!(matches!(
        t.current().resolve_path("gone").unwrap().state(),
        ResolvedPathState::Missing
    ));
    assert!(!t.current().exists("gone").unwrap());
    assert!(t.current().exists("dir").unwrap());
    let expected = t.current().read_expectation("via").unwrap();
    expected.validate(b"data").unwrap();
    assert!(expected.validate(b"date").is_err());
    assert!(expected.validate(b"data!").is_err());
    assert!(t.current().read_expectation("dir").is_err());
    let request = t
        .request(RepositoryWorkspaceOperation::Read("via".into()))
        .unwrap();
    let read = t
        .complete(
            request,
            RepositoryWorkspaceResult::Read(Arc::from(&b"data"[..])),
            t.current().mode(),
            t.current().entries().to_vec(),
        )
        .unwrap();
    assert_eq!(read.retention().body_bytes(), 4);
    assert!(
        read.current().resolution_anchor_storage_bytes().unwrap()
            > std::mem::size_of::<slug_workspace_v2::MaterializationPathAnchor>()
    );
}
#[test]
fn source_link_delete_filters_final_data_but_retains_historical_projection() {
    let t = add_link(
        &transcript(),
        "tool",
        "/__slug_repositories__/source-builtin/dir/body",
        Some(source(21, 7)),
    );
    let req = t
        .request(RepositoryWorkspaceOperation::Delete("tool".into()))
        .unwrap();
    let deleted = t
        .complete(
            req,
            RepositoryWorkspaceResult::Delete(true),
            t.current().mode(),
            vec![root()],
        )
        .unwrap();
    assert!(deleted.current().source_bindings().is_empty());
    let mut seen = 0;
    deleted.visit_read_views(&mut |_| seen += 1);
    assert!(seen >= 2);
    assert!(deleted.storage_partition().unwrap().source_state_bytes() > 0);
    assert_eq!(deleted.retention().body_bytes(), 0);
    assert!(
        deleted.storage_partition().unwrap().owned_bytes()
            < deleted.retained_storage_bytes().unwrap()
    );
}
#[test]
fn payload_links_use_only_exact_execute_prefix_not_future_source_discovery() {
    let t = transcript();
    let view = source(21, 7);
    let mut entries = vec![
        root(),
        link("payload", "/__slug_repositories__/source-builtin/dir/body"),
    ];
    let req = t.request(execute()).unwrap();
    let result = RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
        0,
        Arc::from([]),
        Arc::from([]),
    ));
    assert_eq!(
        t.complete(
            req,
            result.clone(),
            RepositoryWorkspaceMode::Process,
            entries.clone()
        )
        .unwrap_err(),
        RepositoryWorkspaceError::InvalidPath
    );
    let RepositoryWorkspaceOperation::Execute(exec) = execute() else {
        unreachable!()
    };
    let operation = RepositoryWorkspaceOperation::Execute(
        exec.with_read_views(
            RepositoryWorkspaceReadViewManifest::new(t.reservation(), vec![view.clone()]).unwrap(),
        )
        .unwrap(),
    );
    let req = t.request(operation).unwrap();
    let done = t
        .complete(
            req,
            result,
            RepositoryWorkspaceMode::Process,
            std::mem::take(&mut entries),
        )
        .unwrap();
    assert_eq!(done.current().source_bindings(), &[view]);
    assert_eq!(done.retention().body_bytes(), 0);
    assert!(
        t.request_at(0, execute())
            .and_then(|r| done.replay(&r).map(|_| ()))
            .is_err()
    );
}
#[test]
fn typed_symlink_source_requires_independent_declaration_and_full_binding() {
    let t = transcript();
    let view = source(21, 7);
    let request = Arc::new(
        t.request(RepositoryWorkspaceOperation::Symlink {
            path: "x".into(),
            target: NormalizedAbsolutePath::new("/__slug_repositories__/source-builtin/dir/body")
                .unwrap(),
            source: Some(view.clone()),
        })
        .unwrap(),
    );
    assert!(RepositoryWorkspaceOperationDemand::new(request.clone(), Arc::from([])).is_err());
    RepositoryWorkspaceOperationDemand::new_with_sources(
        request.clone(),
        Arc::from([]),
        Arc::from([view.source_state().unwrap().clone()]),
    )
    .unwrap();
    assert!(
        RepositoryWorkspaceOperationDemand::new_with_sources(
            request,
            Arc::from([]),
            Arc::from([source(22, 7).source_state().unwrap().clone()])
        )
        .is_err()
    );
    assert!(
        t.request(RepositoryWorkspaceOperation::Symlink {
            path: "x".into(),
            target: NormalizedAbsolutePath::new("/__slug_repositories__/source-builtin/dir/body")
                .unwrap(),
            source: None
        })
        .is_err()
    );
    assert!(
        t.request(RepositoryWorkspaceOperation::Symlink {
            path: "x".into(),
            target: NormalizedAbsolutePath::new("/__slug_repositories__/source-builtin/dir/body")
                .unwrap(),
            source: Some(source(21, 8))
        })
        .is_err()
    );
}
#[test]
fn source_link_literal_escape_cycles_and_write_through_directory_refuse() {
    let t = add_link(
        &transcript(),
        "dir",
        "/__slug_repositories__/source-builtin/dir",
        Some(source(21, 7)),
    );
    let req = t
        .request(RepositoryWorkspaceOperation::File {
            path: "dir/overwrite".into(),
            body: Arc::from(&b"bad"[..]),
            mode: 0o644,
        })
        .unwrap();
    let mut entries = t.current().entries().to_vec();
    entries.push(file("dir/overwrite", Arc::from(&b"bad"[..])));
    assert!(
        t.complete(
            req,
            RepositoryWorkspaceResult::Unit,
            t.current().mode(),
            entries
        )
        .is_err()
    );
    let candidates = vec![source(21, 7)];
    for target in [
        "/__slug_repositories__/source-builtin/../builtin/dir/body",
        "/foreign/dir/body",
        "../../__slug_repositories__/source-builtin/dir/body",
    ] {
        assert!(
            RepositoryWorkspaceState::bindings_for_entries(
                t.reservation(),
                &[root(), link("bad", target)],
                &candidates
            )
            .is_err()
                || RepositoryWorkspaceState::validate_base(
                    t.reservation(),
                    t.current().mode(),
                    &[root(), link("bad", target)]
                )
                .is_err()
        );
    }
    let cycle = add_link(&transcript(), "loop", "/logical/repo/loop", None);
    assert!(cycle.current().resolve_path("loop").is_err());
}

#[test]
fn source_link_identity_and_callback_include_full_projection_not_just_target_text() {
    let a = source(21, 7);
    let original = a.source_state().unwrap();
    let mut entries = original.entries().to_vec();
    entries[2] =
        SourceEntry::new(entries[2].path().into(), 0o755, entries[2].node().clone()).unwrap();
    let b = RepositoryWorkspaceReadView::from_source(Arc::new(
        RepositorySourceSnapshotState::new(original.reservation().clone(), entries).unwrap(),
    ))
    .unwrap();
    let t = transcript();
    let make = |view| {
        t.request(RepositoryWorkspaceOperation::Symlink {
            path: "x".into(),
            target: NormalizedAbsolutePath::new("/__slug_repositories__/source-builtin/dir/body")
                .unwrap(),
            source: Some(view),
        })
        .unwrap()
    };
    let request = make(a.clone());
    let other = make(b.clone());
    assert_ne!(request, other);
    let mut source_edges = 0;
    let equal = request
        .try_eq_with_states(&other, &mut |left, right| -> Result<bool, ()> {
            match (left, right) {
                (
                    RepositoryWorkspaceReadViewStateRef::Workspace(a),
                    RepositoryWorkspaceReadViewStateRef::Workspace(b),
                ) => Ok(a == b),
                (
                    RepositoryWorkspaceReadViewStateRef::Source(a),
                    RepositoryWorkspaceReadViewStateRef::Source(b),
                ) => {
                    source_edges += 1;
                    Ok(a == b)
                }
                _ => Ok(false),
            }
        })
        .unwrap();
    assert!(!equal);
    assert_eq!(source_edges, 1);
    let x = add_link(
        &t,
        "x",
        "/__slug_repositories__/source-builtin/dir/body",
        Some(a),
    );
    let y = add_link(
        &t,
        "x",
        "/__slug_repositories__/source-builtin/dir/body",
        Some(b),
    );
    assert_eq!(x.current().entries(), y.current().entries());
    assert_ne!(x.current(), y.current());
    assert_ne!(
        x.realize(x.records().len()).unwrap(),
        y.realize(y.records().len()).unwrap()
    );
}

#[test]
fn source_link_content_identity_ignores_fresh_incarnations_at_same_literal_display() {
    let a = add_link(
        &transcript(),
        "tool",
        "/__slug_repositories__/source-builtin/dir/body",
        Some(source(21, 7)),
    );
    let fresh =
        RepositoryWorkspaceTranscript::new(reservation_with(8, 12, limits()), 0o755).unwrap();
    let b = add_link(
        &fresh,
        "tool",
        "/__slug_repositories__/source-builtin/dir/body",
        Some(source(22, 8)),
    );
    assert_ne!(a.current(), b.current());
    assert_ne!(
        a.current().source_bindings()[0],
        b.current().source_bindings()[0]
    );
    let a = a.realize(a.records().len()).unwrap();
    let b = b.realize(b.records().len()).unwrap();
    assert_eq!(a.source_identity(), b.source_identity());
    assert_ne!(a, b);
}

#[test]
fn source_link_content_identity_hashes_full_image_and_prunes_deleted_source_images() {
    let view = source(21, 7);
    let original = view.source_state().unwrap();
    let baseline = add_link(
        &transcript(),
        "tool",
        "/__slug_repositories__/source-builtin/dir/body",
        Some(view.clone()),
    );
    let baseline_id = baseline
        .realize(baseline.records().len())
        .unwrap()
        .source_identity()
        .clone();
    let unlink = |t: &RepositoryWorkspaceTranscript| {
        t.complete(
            t.request(RepositoryWorkspaceOperation::Delete("tool".into()))
                .unwrap(),
            RepositoryWorkspaceResult::Delete(true),
            t.current().mode(),
            vec![root()],
        )
        .unwrap()
    };
    let deleted = unlink(&baseline);
    let deleted_realization = deleted.realize(deleted.records().len()).unwrap();
    assert!(deleted.current().source_bindings().is_empty());
    for change in 0..4 {
        let mut entries = original.entries().to_vec();
        match change {
            0 => {
                entries[2] = SourceEntry::new(
                    "dir/body".into(),
                    0o644,
                    SourceNode::File(
                        FileContentDigest::new(Sha256::digest(b"date").into(), 4).unwrap(),
                    ),
                )
                .unwrap()
            }
            1 => {
                entries[2] =
                    SourceEntry::new("dir/body".into(), 0o755, entries[2].node().clone()).unwrap()
            }
            2 => {
                entries[3] = SourceEntry::new(
                    "internal".into(),
                    0o777,
                    SourceNode::Link("./dir/body".into()),
                )
                .unwrap()
            }
            3 => {
                entries[2] = SourceEntry::new(
                    "dir/body".into(),
                    0o644,
                    SourceNode::File(
                        FileContentDigest::new(Sha256::digest(b"data").into(), 5).unwrap(),
                    ),
                )
                .unwrap()
            }
            _ => unreachable!(),
        }
        let changed = RepositoryWorkspaceReadView::from_source(Arc::new(
            RepositorySourceSnapshotState::new(original.reservation().clone(), entries).unwrap(),
        ))
        .unwrap();
        let changed = add_link(
            &transcript(),
            "tool",
            "/__slug_repositories__/source-builtin/dir/body",
            Some(changed),
        );
        assert_eq!(baseline.current().entries(), changed.current().entries());
        assert_ne!(
            &baseline_id,
            changed
                .realize(changed.records().len())
                .unwrap()
                .source_identity(),
            "source image field {change}"
        );
        let changed = unlink(&changed);
        assert!(changed.current().source_bindings().is_empty());
        let changed_realization = changed.realize(changed.records().len()).unwrap();
        assert_eq!(
            deleted_realization.source_identity(),
            changed_realization.source_identity(),
            "deleted source image field {change}"
        );
        assert_ne!(
            deleted_realization, changed_realization,
            "historical field {change} remains structurally retained"
        );
    }
}

#[test]
fn source_link_modeled_resolution_guard_stops_before_fact_and_reuses_bounded_read_route() {
    let t = add_link(
        &transcript(),
        "dir",
        "/__slug_repositories__/source-builtin/dir",
        Some(source(21, 7)),
    );
    let mut demands = 0;
    let stopped = t.current().resolve_path_with("dir/body", |_| {
        demands += 1;
        if demands == 3 {
            Err("canceled")
        } else {
            Ok(())
        }
    });
    assert_eq!(stopped, Err("canceled"));
    assert_eq!(demands, 3);
    let mut demands = 0;
    let resolved = t
        .current()
        .resolve_path_with("dir/body", |demand| -> Result<(), ()> {
            demands += 1;
            assert!(matches!(
                demand.operation(),
                slug_workspace_v2::PathObservationOperation::Lstat
                    | slug_workspace_v2::PathObservationOperation::ReadLink
            ));
            Ok(())
        })
        .unwrap()
        .unwrap();
    assert!(demands > 3);
    t.current()
        .read_expectation_for_resolved(&resolved)
        .unwrap()
        .validate(b"data")
        .unwrap();
    assert_eq!(resolved, t.current().resolve_path("dir/body").unwrap());
    assert!(
        transcript()
            .current()
            .read_expectation_for_resolved(&resolved)
            .is_err()
    );
}
