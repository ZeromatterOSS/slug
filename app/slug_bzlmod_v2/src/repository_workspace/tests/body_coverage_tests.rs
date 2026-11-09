//! Pure allocation forecasts, not native coverage/permission constructors.
use super::*;

fn difference(
    full: RepositoryWorkspaceStoragePartition,
    covered: RepositoryWorkspaceStoragePartition,
) -> usize {
    assert_eq!(full.source_state_bytes(), covered.source_state_bytes());
    full.owned_bytes()
        .checked_sub(covered.owned_bytes())
        .unwrap()
}

fn bodies(
    walk: impl FnOnce(&mut dyn FnMut(&RepositoryWorkspaceBody) -> Result<(), ()>) -> Result<(), ()>,
) -> Vec<RepositoryWorkspaceBody> {
    let mut found = Vec::new();
    walk(&mut |body| {
        found.push(body.clone());
        Ok(())
    })
    .unwrap();
    found
}

fn other_reservation() -> Arc<RepositoryWorkspaceReservation> {
    let mut other = reservation_with(7, 12, limits()).as_ref().clone();
    other.repository = CanonicalRepoName::new("other+repo").unwrap();
    other.ordinal = 4;
    other.display_root = NormalizedAbsolutePath::new("/logical/other").unwrap();
    Arc::new(other)
}

#[test]
fn body_covered_forecasts_remove_payload_only_and_preserve_full_bounds() {
    let before = transcript();
    let input: RepositoryWorkspaceBody =
        RepositoryWorkspaceBody::from_arc(Arc::from(&b"payload"[..]));
    let foreign: RepositoryWorkspaceBody =
        RepositoryWorkspaceBody::from_arc(Arc::from(input.raw_arc().unwrap().as_ref()));
    assert!(!input.ptr_eq(&foreign));
    let request = before
        .request(RepositoryWorkspaceOperation::File {
            path: "out".into(),
            body: input.clone(),
            mode: 0o644,
        })
        .unwrap();
    let entries = vec![root(), file("out", foreign.clone())];
    let full = before
        .completion_storage_partition(
            &request,
            &RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            &entries,
        )
        .unwrap();
    let covered = before
        .completion_body_covered_storage_partition(
            &request,
            &RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            &entries,
        )
        .unwrap();
    assert_eq!(difference(full, covered), 2 * input.len());
    let incoming = bodies(|v| {
        before.try_visit_completion_body_allocations(
            &request,
            &RepositoryWorkspaceResult::Unit,
            &entries,
            &mut |b| v(b),
        )
    });
    assert_eq!(incoming.len(), 2);
    assert!(incoming[0].ptr_eq(&input));
    assert!(incoming[1].ptr_eq(&foreign));
    let completed = before
        .complete(
            request,
            RepositoryWorkspaceResult::Unit,
            RepositoryWorkspaceMode::Archive,
            entries,
        )
        .unwrap();
    assert_eq!(completed.retention().body_bytes(), input.len() as u64);
    assert_eq!(
        difference(
            completed.storage_partition().unwrap(),
            completed.body_covered_storage_partition().unwrap()
        ),
        input.len()
    );
    assert_eq!(
        difference(
            completed.current().storage_partition().unwrap(),
            completed
                .current()
                .body_covered_storage_partition()
                .unwrap()
        ),
        input.len()
    );
    let canonical = bodies(|v| completed.try_visit_body_allocations(&mut |b| v(b)));
    assert_eq!(canonical.len(), 2);
    assert!(canonical.iter().all(|body| body.ptr_eq(&input)));
    assert_eq!(
        before
            .completion_storage_partition(
                &before
                    .request(RepositoryWorkspaceOperation::File {
                        path: "out".into(),
                        body: input.clone(),
                        mode: 0o644
                    })
                    .unwrap(),
                &RepositoryWorkspaceResult::Unit,
                RepositoryWorkspaceMode::Archive,
                &[root(), file("out", foreign)]
            )
            .unwrap(),
        full
    );
    let realization = completed.realize(1).unwrap();
    assert_eq!(
        difference(
            realization.storage_partition().unwrap(),
            realization.body_covered_storage_partition().unwrap()
        ),
        input.len()
    );
    assert_eq!(
        difference(
            completed.realization_storage_partition().unwrap(),
            completed
                .realization_body_covered_storage_partition()
                .unwrap()
        ),
        input.len()
    );
    let terminal = crate::RepositoryMaterializationRequest {
        id: crate::RepositoryMaterializationRequestId {
            workspace: realization.reservation().workspace().clone(),
            canonical_repo: realization.reservation().repository().clone(),
        },
        repo_spec: realization
            .reservation()
            .expected_repo_spec()
            .as_ref()
            .clone(),
        kind: crate::RepositoryMaterializationKind::GeneralWorkspace(realization),
    };
    assert_eq!(
        difference(
            terminal.general_workspace_storage_partition().unwrap(),
            terminal
                .general_workspace_body_covered_storage_partition()
                .unwrap()
        ),
        input.len()
    );
    assert_eq!(
        bodies(|v| terminal
            .try_visit_general_workspace_body_allocations(&mut |b| v(b))
            .map(|r| r.unwrap()))
        .len(),
        canonical.len()
    );
}

#[test]
fn body_visitors_cover_template_read_and_deleted_history_but_not_execute_outputs() {
    let written = write(&transcript(), b"historical").unwrap();
    let read = written
        .complete(
            written
                .request(RepositoryWorkspaceOperation::Read("out".into()))
                .unwrap(),
            RepositoryWorkspaceResult::Read(RepositoryWorkspaceBody::from_arc(Arc::from(
                &b"historical"[..],
            ))),
            RepositoryWorkspaceMode::Archive,
            written.current().entries().to_vec(),
        )
        .unwrap();
    let deleted = read
        .complete(
            read.request(RepositoryWorkspaceOperation::Delete("out".into()))
                .unwrap(),
            RepositoryWorkspaceResult::Delete(true),
            RepositoryWorkspaceMode::Archive,
            vec![root()],
        )
        .unwrap();
    assert!(bodies(|v| deleted.current().try_visit_body_allocations(&mut |b| v(b))).is_empty());
    let history = bodies(|v| deleted.try_visit_body_allocations(&mut |b| v(b)));
    assert_eq!(history.len(), 4);
    assert!(history.iter().all(|b| b.equals_slice(b"historical")));
    assert_eq!(
        difference(
            deleted.storage_partition().unwrap(),
            deleted.body_covered_storage_partition().unwrap()
        ),
        b"historical".len()
    );
    let template = RepositoryWorkspaceOperation::Template {
        path: "rendered".into(),
        body: RepositoryWorkspaceBody::from_arc(Arc::from(&b"rendered"[..])),
        mode: 0o644,
        source: host_path("/input"),
        source_body: RepositoryWorkspaceBody::from_arc(Arc::from(&b"template"[..])),
        substitutions: SmallMap::new(),
    };
    assert_eq!(
        bodies(|v| template.try_visit_body_allocations(&mut |b| v(b)))
            .iter()
            .map(|b| b.raw_arc().unwrap().as_ref())
            .collect::<Vec<_>>(),
        [b"rendered".as_slice(), b"template".as_slice()]
    );
    let request = deleted.request(template).unwrap();
    assert_eq!(
        difference(
            request.storage_partition().unwrap(),
            request.body_covered_storage_partition().unwrap()
        ),
        b"rendered".len() + b"template".len()
    );
    let result = RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
        0,
        Arc::from(&b"stdout"[..]),
        Arc::from(&b"stderr"[..]),
    ));
    assert!(bodies(|v| result.try_visit_body_allocations(&mut |b| v(b))).is_empty());
    let base = transcript();
    let request = base.request(execute()).unwrap();
    let full = base
        .completion_storage_partition(
            &request,
            &result,
            RepositoryWorkspaceMode::Process,
            &[root()],
        )
        .unwrap();
    assert_eq!(
        full,
        base.completion_body_covered_storage_partition(
            &request,
            &result,
            RepositoryWorkspaceMode::Process,
            &[root()]
        )
        .unwrap()
    );
    assert_eq!(
        result.metadata().unwrap(),
        array_bytes::<u8>(6).unwrap() * 2
    );
    let complete = base
        .complete(
            request,
            result,
            RepositoryWorkspaceMode::Process,
            vec![root()],
        )
        .unwrap();
    assert_eq!(complete.retention().body_bytes(), 0);
    assert_eq!(
        complete.storage_partition().unwrap(),
        complete.body_covered_storage_partition().unwrap()
    );
}

#[test]
fn epoch_body_projections_replace_only_the_validated_slot_and_preserve_sources() {
    let a0 = transcript();
    let b0 = RepositoryWorkspaceTranscript::new(other_reservation(), 0o755).unwrap();
    let a1 = write(&a0, b"A").unwrap();
    let b1 = write(&b0, b"BBB").unwrap();
    let epoch = RepositoryWorkspaceEpoch::fresh(a0.reservation().workspace().clone(), 7, limits())
        .unwrap()
        .reserve(a0.reservation().clone(), 0o755)
        .unwrap()
        .reserve(b0.reservation().clone(), 0o755)
        .unwrap()
        .publish_completed(a1.clone())
        .unwrap()
        .publish_completed(b1)
        .unwrap();
    let a2 = write(&a1, b"AA").unwrap();
    assert_eq!(
        difference(
            epoch.storage_partition().unwrap(),
            epoch.body_covered_storage_partition().unwrap()
        ),
        4
    );
    assert_eq!(
        difference(
            epoch.publish_storage_partition(&a2).unwrap(),
            epoch.publish_body_covered_storage_partition(&a2).unwrap()
        ),
        6
    );
    let projected = bodies(|v| {
        epoch
            .try_visit_publish_body_allocations(&a2, &mut |b| v(b))
            .map(|r| r.unwrap())
    });
    assert_eq!(projected.len(), 6);
    let mut third = other_reservation().as_ref().clone();
    third.repository = CanonicalRepoName::new("third+repo").unwrap();
    third.instance = PathObservationInstanceId::new(13);
    third.ordinal = 5;
    third.display_root = NormalizedAbsolutePath::new("/logical/third").unwrap();
    assert_eq!(
        difference(
            epoch.reserve_storage_partition(&third, 0o755).unwrap(),
            epoch
                .reserve_body_covered_storage_partition(&third, 0o755)
                .unwrap()
        ),
        4
    );
    assert_eq!(
        bodies(|v| epoch
            .try_visit_reserve_body_allocations(&third, 0o755, &mut |b| v(b))
            .map(|r| r.unwrap()))
        .len(),
        4
    );
    let view = RepositoryWorkspaceReadView::new(
        b0.reservation().expected_repo_spec().as_ref().clone(),
        write(&b0, b"external payload").unwrap().current().clone(),
    )
    .unwrap();
    let RepositoryWorkspaceOperation::Execute(exec) = execute() else {
        unreachable!()
    };
    let request = a1
        .request(RepositoryWorkspaceOperation::Execute(
            exec.with_read_views(
                RepositoryWorkspaceReadViewManifest::new(a1.reservation(), vec![view]).unwrap(),
            )
            .unwrap(),
        ))
        .unwrap();
    let full = request.storage_partition().unwrap();
    assert!(full.source_state_bytes() > b"external payload".len());
    assert_eq!(
        difference(full, request.body_covered_storage_partition().unwrap()),
        1
    );
    assert!(
        bodies(|v| request.try_visit_body_allocations(&mut |b| v(b)))
            .iter()
            .all(|b| b.equals_slice(b"A"))
    );
}

#[test]
fn body_visitors_short_circuit_and_invalid_projections_never_invoke_callbacks() {
    let value = write(&transcript(), b"payload").unwrap();
    let mut calls = 0;
    assert_eq!(
        value.try_visit_body_allocations(&mut |_| {
            calls += 1;
            Err("cancelled")
        }),
        Err("cancelled")
    );
    assert_eq!(calls, 1);
    let epoch =
        RepositoryWorkspaceEpoch::fresh(value.reservation().workspace().clone(), 7, limits())
            .unwrap()
            .reserve(value.reservation().clone(), 0o755)
            .unwrap();
    calls = 0;
    assert_eq!(
        epoch.try_visit_publish_body_allocations(&value, &mut |_| {
            calls += 1;
            Err("cancelled")
        }),
        Err("cancelled")
    );
    assert_eq!(calls, 1);
    calls = 0;
    assert!(
        RepositoryWorkspaceEpoch::revoked()
            .try_visit_publish_body_allocations(&value, &mut |_| {
                calls += 1;
                Ok::<_, ()>(())
            })
            .unwrap()
            .is_err()
    );
    assert!(
        epoch
            .try_visit_reserve_body_allocations(value.reservation(), 0o755, &mut |_| {
                calls += 1;
                Ok::<_, ()>(())
            })
            .unwrap()
            .is_err()
    );
    assert_eq!(calls, 0);
    let bad = value
        .request(RepositoryWorkspaceOperation::Exists("".into()))
        .unwrap();
    assert_eq!(
        value.completion_body_covered_storage_partition(
            &bad,
            &RepositoryWorkspaceResult::Exists(false),
            value.current().mode(),
            value.current().entries()
        ),
        value.completion_storage_partition(
            &bad,
            &RepositoryWorkspaceResult::Exists(false),
            value.current().mode(),
            value.current().entries()
        )
    );
    assert!(
        value
            .completion_storage_partition(
                &bad,
                &RepositoryWorkspaceResult::Exists(false),
                value.current().mode(),
                value.current().entries()
            )
            .is_err()
    );
    assert_eq!(value.records().len(), 1);
}
