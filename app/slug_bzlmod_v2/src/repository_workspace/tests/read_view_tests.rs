use super::*;
use crate::RepoRuleId;
use crate::RepoSpec;

fn spec() -> RepoSpec {
    RepoSpec {
        rule_id: RepoRuleId {
            bzl_file: CanonicalLabel::parse("@@//:repos.bzl").unwrap(),
            rule_name: "repository".into(),
        },
        attributes: Arc::new(SmallMap::new()),
    }
}
fn source_reservation(
    name: &str,
    instance: u64,
    display: &str,
) -> Arc<RepositoryWorkspaceReservation> {
    let executor = reservation_with(7, 11, limits());
    Arc::new(
        RepositoryWorkspaceReservation::new(
            executor.workspace().clone(),
            executor.owner().clone(),
            CanonicalRepoName::new(name).unwrap(),
            instance as usize,
            Arc::new(spec()),
            NormalizedAbsolutePath::new(display).unwrap(),
            PathObservationInstanceId::new(instance),
            7,
            limits(),
        )
        .unwrap(),
    )
}
fn source(
    reservation: Arc<RepositoryWorkspaceReservation>,
    body: Arc<[u8]>,
) -> RepositoryWorkspaceTranscript {
    let t = RepositoryWorkspaceTranscript::new(reservation, 0o755).unwrap();
    t.complete(
        t.request(RepositoryWorkspaceOperation::File {
            path: "data".into(),
            body: body.clone(),
            mode: 0o644,
        })
        .unwrap(),
        RepositoryWorkspaceResult::Unit,
        RepositoryWorkspaceMode::Archive,
        vec![root(), file("data", body)],
    )
    .unwrap()
}
fn projection(t: &RepositoryWorkspaceTranscript) -> RepositoryWorkspaceReadView {
    RepositoryWorkspaceReadView::new(spec(), t.current().clone()).unwrap()
}
fn projection_with_spec(
    state: &RepositoryWorkspaceState,
    repo_spec: RepoSpec,
) -> RepositoryWorkspaceReadView {
    let mut reservation = state.reservation().as_ref().clone();
    reservation.expected_repo_spec = Arc::new(repo_spec.clone());
    let state = RepositoryWorkspaceState::from_admitted(
        Arc::new(reservation),
        state.mode(),
        state.entries().to_vec(),
    );
    RepositoryWorkspaceReadView::new(repo_spec, Arc::new(state)).unwrap()
}
fn manifest(views: Vec<RepositoryWorkspaceReadView>) -> RepositoryWorkspaceReadViewManifest {
    RepositoryWorkspaceReadViewManifest::new(&reservation_with(7, 11, limits()), views).unwrap()
}
fn operation(views: RepositoryWorkspaceReadViewManifest) -> RepositoryWorkspaceOperation {
    let RepositoryWorkspaceOperation::Execute(command) = execute() else {
        unreachable!()
    };
    RepositoryWorkspaceOperation::Execute(command.with_read_views(views).unwrap())
}
fn complete_execute(
    t: &RepositoryWorkspaceTranscript,
    views: RepositoryWorkspaceReadViewManifest,
) -> RepositoryWorkspaceTranscript {
    t.complete(
        t.request(operation(views)).unwrap(),
        RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
            0,
            Arc::from([]),
            Arc::from([]),
        )),
        RepositoryWorkspaceMode::Process,
        vec![root()],
    )
    .unwrap()
}

#[test]
fn manifest_canonicalizes_exact_bound_and_refuses_aliases_and_foreign_command_facts() {
    let executor = reservation_with(7, 11, limits());
    let rows = || {
        (0..128)
            .rev()
            .map(|i| {
                let r = source_reservation(
                    &format!("source{i:03}"),
                    100 + i,
                    &format!("/views/{i:03}"),
                );
                projection(&RepositoryWorkspaceTranscript::new(r, 0o755).unwrap())
            })
            .collect::<Vec<_>>()
    };
    let canonical = RepositoryWorkspaceReadViewManifest::new(&executor, rows()).unwrap();
    assert_eq!(canonical.views().len(), 128);
    assert!(canonical.views().windows(2).all(|v| {
        v[0].workspace_state().unwrap().reservation().display_root()
            < v[1].workspace_state().unwrap().reservation().display_root()
    }));
    let mut over = rows();
    over.push(projection(
        &RepositoryWorkspaceTranscript::new(
            source_reservation("extra", 999, "/views/extra"),
            0o755,
        )
        .unwrap(),
    ));
    assert_eq!(
        RepositoryWorkspaceReadViewManifest::new(&executor, over).unwrap_err(),
        RepositoryWorkspaceError::StateLimit
    );
    let base = source_reservation("source", 100, "/views/source");
    let row = projection(&RepositoryWorkspaceTranscript::new(base.clone(), 0o755).unwrap());
    for conflict in [
        source_reservation("source", 101, "/views/other"),
        source_reservation("other", 100, "/views/other"),
        source_reservation("other", 101, "/views/source"),
        source_reservation("other", 101, "/views/source/child"),
        source_reservation("other", 101, "/views"),
    ] {
        let other = projection(&RepositoryWorkspaceTranscript::new(conflict, 0o755).unwrap());
        assert_eq!(
            RepositoryWorkspaceReadViewManifest::new(&executor, vec![row.clone(), other])
                .unwrap_err(),
            RepositoryWorkspaceError::ForeignReservation
        );
    }
    for (workspace, lease, policy, display) in [
        (
            NormalizedAbsolutePath::new("/foreign").unwrap(),
            7,
            limits(),
            "/views/foreign",
        ),
        (executor.workspace().clone(), 8, limits(), "/views/foreign"),
        (
            executor.workspace().clone(),
            7,
            RepositoryWorkspaceRetentionLimits::new(
                1023,
                limits().metadata_bytes(),
                limits().archive_body_bytes(),
            )
            .unwrap(),
            "/views/foreign",
        ),
        (
            executor.workspace().clone(),
            7,
            limits(),
            "/logical/repo/child",
        ),
    ] {
        let r = Arc::new(
            RepositoryWorkspaceReservation::new(
                workspace,
                base.owner().clone(),
                base.repository().clone(),
                base.ordinal(),
                base.expected_repo_spec().clone(),
                NormalizedAbsolutePath::new(display).unwrap(),
                base.instance(),
                lease,
                policy,
            )
            .unwrap(),
        );
        let view = projection(&RepositoryWorkspaceTranscript::new(r, 0o755).unwrap());
        assert!(RepositoryWorkspaceReadViewManifest::new(&executor, vec![view]).is_err());
    }
    assert!(
        canonical
            .validate_for(&reservation_with(8, 12, limits()))
            .is_err()
    );
}

#[test]
fn view_identity_uses_full_spec_state_mode_literal_link_and_instance() {
    let r = source_reservation("source", 100, "/views/source");
    let a = source(r.clone(), Arc::from(&b"a"[..]));
    let b = source(r.clone(), Arc::from(&b"b"[..]));
    let base = operation(manifest(vec![projection(&a)]));
    assert_ne!(base, operation(manifest(vec![projection(&b)])));
    let mut changed = spec();
    changed.rule_id.rule_name = "different".into();
    assert_ne!(
        base,
        operation(manifest(vec![projection_with_spec(a.current(), changed)]))
    );
    let other = source(
        source_reservation("source", 101, "/views/source"),
        Arc::from(&b"a"[..]),
    );
    assert_ne!(base, operation(manifest(vec![projection(&other)])));
    let command = RepositoryWorkspaceExecute::new(
        Arc::from([RepositoryWorkspaceArgument::String("helper".into())]),
        RepositoryEnvironmentSnapshot::empty(),
        SmallMap::new(),
        600,
        true,
        HostRepositoryLabelPathValue::new(
            r.display_root().clone(),
            PathObservationNamespace::Materialization(r.instance()),
        )
        .unwrap(),
        RepositoryWorkspaceReadViewManifest::empty(&r),
    )
    .unwrap();
    let process = a
        .complete(
            a.request(RepositoryWorkspaceOperation::Execute(command))
                .unwrap(),
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([]),
            )),
            RepositoryWorkspaceMode::Process,
            a.current().entries().to_vec(),
        )
        .unwrap();
    assert_ne!(base, operation(manifest(vec![projection(&process)])));
    let with_link = |target: &str| {
        a.complete(
            a.request(
                process
                    .records()
                    .last()
                    .unwrap()
                    .request()
                    .operation()
                    .clone(),
            )
            .unwrap(),
            RepositoryWorkspaceResult::Execute(RepositoryExecuteResult::new(
                0,
                Arc::from([]),
                Arc::from([]),
            )),
            RepositoryWorkspaceMode::Process,
            vec![
                root(),
                file("data", Arc::from(&b"a"[..])),
                RepositoryWorkspaceEntry::new(
                    "link".into(),
                    0o777,
                    RepositoryWorkspaceNode::Link(target.into()),
                )
                .unwrap(),
            ],
        )
        .unwrap()
    };
    assert_ne!(
        operation(manifest(vec![projection(&with_link("data"))])),
        operation(manifest(vec![projection(&with_link("./data"))]))
    );
}

#[test]
fn exact_prefix_replay_keeps_old_views_and_rehomes_equal_foreign_state_without_copying() {
    let r = source_reservation("source", 100, "/views/source");
    let a = source(r.clone(), Arc::from(&b"same"[..]));
    let foreign = source(r, Arc::from(&b"same"[..]));
    assert_eq!(a.current(), foreign.current());
    assert!(!Arc::ptr_eq(a.current(), foreign.current()));
    let selected = manifest(vec![projection(&a)]);
    let t = complete_execute(&transcript(), selected.clone());
    let request = t
        .request_at(0, operation(manifest(vec![projection(&foreign)])))
        .unwrap();
    assert!(t.replay(&request).unwrap().is_some());
    let canonical = request.with_read_views(selected.clone()).unwrap();
    assert_eq!(t.records()[0].request(), &canonical);
    canonical.visit_read_views(&mut |view| {
        assert!(Arc::ptr_eq(view.workspace_state().unwrap(), a.current()))
    });
    let b = source(
        source_reservation("later", 101, "/views/later"),
        Arc::from(&b"later"[..]),
    );
    let expanded = manifest(vec![projection(&b), projection(&a)]);
    let changed = t.request_at(0, operation(expanded.clone())).unwrap();
    assert_eq!(
        t.replay(&changed).unwrap_err(),
        RepositoryWorkspaceError::ReplayConflict
    );
    let next = complete_execute(&t, expanded);
    let RepositoryWorkspaceOperation::Execute(old) = next.records()[0].request().operation() else {
        unreachable!()
    };
    let RepositoryWorkspaceOperation::Execute(new) = next.records()[1].request().operation() else {
        unreachable!()
    };
    assert_eq!(old.read_views().views().len(), 1);
    assert_eq!(new.read_views().views().len(), 2);
    assert!(next.replay(&canonical).unwrap().is_some());
}

#[test]
fn archive_source_storage_stays_outside_mutable_history_and_forecasts_remain_complete() {
    let body: Arc<[u8]> = Arc::from(vec![0x5a; 65 * 1024 * 1024]);
    let source = source(
        source_reservation("large", 100, "/views/large"),
        body.clone(),
    );
    let selected = manifest(vec![projection(&source)]);
    let request = transcript().request(operation(selected.clone())).unwrap();
    let part = request.storage_partition().unwrap();
    assert!(part.source_state_bytes() >= body.len());
    assert!(part.owned_bytes() < 1024 * 1024);
    assert_eq!(
        request.retained_storage_bytes().unwrap(),
        part.total_bytes().unwrap()
    );
    let t = complete_execute(&transcript(), selected);
    assert_eq!(t.retention().body_bytes(), 0);
    let full = t.storage_partition().unwrap();
    assert!(full.source_state_bytes() >= body.len());
    assert!(full.owned_bytes() < 1024 * 1024);
    assert_eq!(
        t.retained_storage_bytes().unwrap(),
        full.total_bytes().unwrap()
    );
    let realized = t.realize(1).unwrap();
    assert_eq!(
        realized.storage_partition().unwrap().source_state_bytes(),
        full.source_state_bytes()
    );
    assert!(
        t.realization_storage_partition().unwrap().owned_bytes()
            >= realized.storage_partition().unwrap().owned_bytes()
    );
    t.visit_read_views(&mut |view| {
        let RepositoryWorkspaceNode::File(retained) = view
            .workspace_state()
            .unwrap()
            .entry("data")
            .unwrap()
            .node()
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(retained, &body));
    });
}

#[test]
fn repeated_view_refs_charge_specs_and_refs_but_never_repeat_mutable_bodies() {
    let source = source(
        source_reservation("source", 100, "/views/source"),
        Arc::from(vec![1u8; 1024 * 1024]),
    );
    let selected = manifest(vec![projection(&source)]);
    let mut t = transcript();
    for _ in 0..128 {
        t = complete_execute(&t, selected.clone());
    }
    assert_eq!(t.retention().body_bytes(), 0);
    let storage = t.storage_partition().unwrap();
    assert!(storage.source_state_bytes() >= 128 * 1024 * 1024);
    assert!(storage.owned_bytes() < 8 * 1024 * 1024);
    let mut refs = 0;
    t.visit_read_views(&mut |view| {
        refs += 1;
        assert!(Arc::ptr_eq(
            view.workspace_state().unwrap(),
            source.current()
        ));
    });
    assert_eq!(refs, 128);
    let mut spare = CompactString::with_capacity(32768);
    spare.push_str("value");
    let mut larger = spec();
    larger.attributes = Arc::new(SmallMap::from_iter([(
        CompactString::new("key"),
        crate::OverrideAttributeValue::String(spare),
    )]));
    let larger = manifest(vec![projection_with_spec(source.current(), larger)]);
    assert!(
        larger.storage_partition().unwrap().owned_bytes()
            > selected.storage_partition().unwrap().owned_bytes() + 32000
    );
    let shared: Arc<[crate::OverrideAttributeValue]> = Arc::from(
        (0..65535)
            .map(|_| crate::OverrideAttributeValue::None)
            .collect::<Vec<_>>(),
    );
    let mut large_spec = spec();
    large_spec.attributes = Arc::new(SmallMap::from_iter([(
        CompactString::new("key"),
        crate::OverrideAttributeValue::Iterable(shared),
    )]));
    let a = projection_with_spec(source.current(), large_spec.clone());
    let other = source_reservation("other", 101, "/views/other");
    let b = projection_with_spec(
        RepositoryWorkspaceTranscript::new(other, 0o755)
            .unwrap()
            .current(),
        large_spec,
    );
    assert_eq!(
        RepositoryWorkspaceReadViewManifest::new(&reservation_with(7, 11, limits()), vec![a, b])
            .unwrap_err(),
        RepositoryWorkspaceError::RetentionLimit
    );
}

#[test]
fn bounded_structural_comparison_delegates_every_state_edge_and_preserves_other_fields() {
    let reservation = source_reservation("source", 100, "/views/source");
    let a = source(reservation.clone(), Arc::from(&b"same"[..]));
    let foreign = source(reservation, Arc::from(&b"same"[..]));
    let selected = manifest(vec![projection(&a)]);
    let t = complete_execute(&transcript(), selected.clone());
    let other = complete_execute(&transcript(), manifest(vec![projection(&foreign)]));
    assert_eq!(t, other);
    let mut calls = 0;
    let mut source_calls = 0;
    let equal = t.try_eq_with_states(&other, &mut |left, right| {
        calls += 1;
        let RepositoryWorkspaceReadViewStateRef::Workspace(left_state) = left else {
            panic!("workspace fixture")
        };
        let RepositoryWorkspaceReadViewStateRef::Workspace(right_state) = right else {
            panic!("workspace fixture")
        };
        if left_state.reservation().repository().as_str() == "source" {
            source_calls += 1;
            assert!(!Arc::ptr_eq(left_state, right_state));
        }
        Ok::<_, ()>(left == right)
    });
    // Initial, record predecessor, Execute source view and record next.
    assert_eq!(equal, Ok(true));
    assert_eq!((calls, source_calls), (4, 1));
    let request = t.records()[0].request();
    let foreign_request = other.records()[0].request();
    let mut calls = 0;
    assert_eq!(
        request.try_eq_with_states(foreign_request, &mut |a, b| {
            calls += 1;
            Ok::<_, ()>(a == b)
        }),
        Ok(true)
    );
    assert_eq!(calls, 2);
    assert_eq!(
        t.try_eq_with_states(&other, &mut |_, _| Err::<bool, _>(17u8)),
        Err(17)
    );
    let realization = t.realize(1).unwrap();
    let foreign_realization = other.realize(1).unwrap();
    let mut calls = 0;
    assert_eq!(
        realization.try_eq_with_states(&foreign_realization, &mut |a, b| {
            calls += 1;
            Ok::<_, ()>(a == b)
        }),
        Ok(true)
    );
    assert_eq!(calls, 4);
    let terminal = |realization| crate::RepositoryMaterializationRequest {
        id: crate::RepositoryMaterializationRequestId {
            workspace: t.reservation().workspace().clone(),
            canonical_repo: t.reservation().repository().clone(),
        },
        repo_spec: spec(),
        kind: crate::RepositoryMaterializationKind::GeneralWorkspace(realization),
    };
    let terminal_a = terminal(realization);
    let terminal_b = terminal(foreign_realization);
    let mut calls = 0;
    assert_eq!(
        terminal_a.try_eq_with_states(&terminal_b, &mut |a, b| {
            calls += 1;
            Ok::<_, ()>(a == b)
        }),
        Ok(true)
    );
    assert_eq!(calls, 4);
    let epoch_entry = |request: &crate::RepositoryMaterializationRequest| {
        let crate::RepositoryMaterializationKind::GeneralWorkspace(realization) = &request.kind
        else {
            unreachable!()
        };
        crate::RepositoryMaterializationEpochEntry {
            request: Arc::new(request.clone()),
            result: crate::RepositoryMaterializationResult::Success(
                crate::RepositoryMaterializationSuccess::Immutable {
                    path_policy: crate::RepositoryMaterializationPathPolicy::GenerationRoot,
                    source_identity: realization.source_identity().clone(),
                    generation_root: realization
                        .reservation()
                        .display_root()
                        .as_path()
                        .to_path_buf(),
                    observation_instance: realization.reservation().instance(),
                },
            ),
        }
    };
    let epoch_a = crate::RepositoryMaterializationResultEpoch::new(
        t.reservation().workspace().clone(),
        [epoch_entry(&terminal_a)],
    )
    .unwrap();
    let epoch_b = crate::RepositoryMaterializationResultEpoch::new(
        t.reservation().workspace().clone(),
        [epoch_entry(&terminal_b)],
    )
    .unwrap();
    let mut calls = 0;
    assert_eq!(
        epoch_a.try_eq_with_states(&epoch_b, &mut |a, b| {
            calls += 1;
            Ok::<_, ()>(a == b)
        }),
        Ok(true)
    );
    assert_eq!(calls, 4);
    assert_eq!(
        epoch_a.try_eq_with_states(&epoch_b, &mut |_, _| Err::<bool, _>(19u8)),
        Err(19)
    );
    let mut stale = epoch_entry(&terminal_b);
    stale.result = crate::RepositoryMaterializationResult::MaterializationError {
        generation: crate::RepositoryMaterializationGeneration(99),
        message: "stale".into(),
    };
    let stale = crate::RepositoryMaterializationResultEpoch::new(
        t.reservation().workspace().clone(),
        [stale],
    )
    .unwrap();
    assert_ne!(epoch_a, stale);
    assert_eq!(
        epoch_a.try_eq_with_states(&stale, &mut |_, _| Ok::<_, ()>(true)),
        Ok(false)
    );
    let foreign_epoch = crate::RepositoryMaterializationResultEpoch::new(
        NormalizedAbsolutePath::new("/foreign").unwrap(),
        [],
    )
    .unwrap();
    assert_eq!(
        epoch_a.try_eq_with_states(&foreign_epoch, &mut |_, _| Ok::<_, ()>(true)),
        Ok(false)
    );
    let retained_entry = crate::RepositoryMaterializationEpochEntry {
        request: Arc::new(crate::RepositoryMaterializationRequest {
            id: crate::RepositoryMaterializationRequestId {
                workspace: t.reservation().workspace().clone(),
                canonical_repo: CanonicalRepoName::new("retained").unwrap(),
            },
            repo_spec: spec(),
            kind: crate::RepositoryMaterializationKind::GeneratedFileEffects(
                crate::GeneratedRepositoryFileEffectPlan::build(std::iter::empty::<(
                    CompactString,
                    Arc<[u8]>,
                    bool,
                )>())
                .unwrap(),
            ),
        }),
        result: crate::RepositoryMaterializationResult::Success(
            crate::RepositoryMaterializationSuccess::Immutable {
                path_policy: crate::RepositoryMaterializationPathPolicy::LegacyUnbounded,
                source_identity: Arc::from("retained"),
                generation_root: "/retained/root".into(),
                observation_instance: PathObservationInstanceId::new(777),
            },
        ),
    };
    let without_inventory = crate::RepositoryMaterializationResultEpoch::new(
        t.reservation().workspace().clone(),
        [retained_entry],
    )
    .unwrap();
    let with_inventory = without_inventory
        .clone()
        .retaining_generated_outputs(None, 128)
        .unwrap();
    assert_ne!(without_inventory, with_inventory);
    assert_eq!(
        without_inventory.try_eq_with_states(&with_inventory, &mut |_, _| Ok::<_, ()>(true)),
        Ok(false)
    );
    assert_eq!(
        with_inventory.try_eq_with_states(&with_inventory.clone(), &mut |_, _| Ok::<_, ()>(true)),
        Ok(true)
    );
    let mut wrong_spec = terminal_b.clone();
    wrong_spec.repo_spec.rule_id.rule_name = "different".into();
    assert_eq!(
        terminal_a.try_eq_with_states(&wrong_spec, &mut |_, _| Ok::<_, ()>(true)),
        Ok(false)
    );
    let mut wrong_id = terminal_b.clone();
    wrong_id.id.canonical_repo = CanonicalRepoName::new("different").unwrap();
    assert_eq!(
        terminal_a.try_eq_with_states(&wrong_id, &mut |_, _| Ok::<_, ()>(true)),
        Ok(false)
    );

    let RepositoryWorkspaceOperation::Execute(base) = request.operation() else {
        unreachable!()
    };
    for field in 0..7 {
        let argv: Arc<[RepositoryWorkspaceArgument]> = if field == 0 {
            Arc::from([RepositoryWorkspaceArgument::String("different".into())])
        } else {
            Arc::from(base.argv())
        };
        let environment = if field == 1 {
            RepositoryEnvironmentSnapshot::from_canonical(Arc::from([
                RepositoryEnvironmentEntry::new("DIFFERENT", "value"),
            ]))
            .unwrap()
        } else {
            base.environment().clone()
        };
        let mut overrides = base.overrides().clone();
        if field == 2 {
            overrides.insert(
                "DIFFERENT".into(),
                RepositoryWorkspaceEnvironmentValue::Override("value".into()),
            );
        }
        let cwd = if field == 5 {
            HostRepositoryLabelPathValue::new(
                NormalizedAbsolutePath::new("/logical/repo/subdir").unwrap(),
                base.working_directory().namespace(),
            )
            .unwrap()
        } else {
            base.working_directory().clone()
        };
        let views = if field == 6 {
            let mut changed_spec = spec();
            changed_spec.rule_id.rule_name = "different".into();
            manifest(vec![projection_with_spec(a.current(), changed_spec)])
        } else {
            selected.clone()
        };
        let changed = RepositoryWorkspaceExecute::new(
            argv,
            environment,
            overrides,
            if field == 3 {
                599
            } else {
                base.timeout_seconds()
            },
            if field == 4 {
                !base.quiet()
            } else {
                base.quiet()
            },
            cwd,
            views,
        )
        .unwrap();
        let changed = transcript()
            .request(RepositoryWorkspaceOperation::Execute(changed))
            .unwrap();
        assert_ne!(request, &changed);
        // A permissive state callback must never hide a scalar/Spec difference.
        assert_eq!(
            request.try_eq_with_states(&changed, &mut |_, _| Ok::<_, ()>(true)),
            Ok(false)
        );
    }
    let changed_source = source(a.reservation().clone(), Arc::from(&b"different"[..]));
    let changed = complete_execute(&transcript(), manifest(vec![projection(&changed_source)]));
    let crate::RepositoryMaterializationKind::GeneralWorkspace(original_realization) =
        &terminal_a.kind
    else {
        unreachable!()
    };
    assert_eq!(
        original_realization.source_identity(),
        changed.realize(1).unwrap().source_identity()
    );
    assert_eq!(
        t.try_eq_with_states(&changed, &mut |a, b| Ok::<_, ()>(a == b)),
        Ok(false)
    );
    let later = t.request(operation(selected)).unwrap();
    assert_eq!(
        request.try_eq_with_states(&later, &mut |_, _| Ok::<_, ()>(true)),
        Ok(false)
    );
    assert_eq!(
        t.try_eq_with_states(&transcript(), &mut |_, _| Ok::<_, ()>(true)),
        Ok(false)
    );
}

#[test]
fn source_digest_view_is_exact_bounded_and_not_mutable_process_body_history() {
    use crate::RepositorySourceSnapshotEntry as Entry;
    use crate::RepositorySourceSnapshotNode as Node;
    use crate::RepositorySourceSnapshotRequest as Request;
    use crate::RepositorySourceSnapshotReservation as Reservation;
    use crate::RepositorySourceSnapshotState as State;
    let route = Arc::new(crate::HostCanonicalRepositoryRoute::builtin(
        NormalizedAbsolutePath::new("/workspace").unwrap(),
        crate::HostBuiltinBazelToolsRepositoryMapping::testing(),
    ));
    let input = crate::host_canonical_repository_source_input(route, None).unwrap();
    let request = Arc::new(Request::from_canonical(&input, 7).unwrap());
    let reservation = Arc::new(
        Reservation::new(
            request,
            NormalizedAbsolutePath::new("/__slug_repositories__/v1/builtin").unwrap(),
            PathObservationInstanceId::new(100),
        )
        .unwrap(),
    );
    let make = |digest| {
        Arc::new(
            State::new(
                reservation.clone(),
                vec![
                    Entry::new("".into(), 0o755, Node::Directory).unwrap(),
                    Entry::new(
                        "a".into(),
                        0o644,
                        Node::File(
                            slug_workspace_v2::FileContentDigest::new(digest, 64 * 1024 * 1024)
                                .unwrap(),
                        ),
                    )
                    .unwrap(),
                    Entry::new(
                        "b".into(),
                        0o644,
                        Node::File(
                            slug_workspace_v2::FileContentDigest::new([2; 32], 1024 * 1024)
                                .unwrap(),
                        ),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        )
    };
    let source = make([1; 32]);
    let view = RepositoryWorkspaceReadView::from_source(source.clone()).unwrap();
    assert!(view.workspace_state().is_none());
    assert!(view.repo_spec().is_none());
    assert!(Arc::ptr_eq(view.source_state().unwrap(), &source));
    let partition = view.storage_partition().unwrap();
    assert!(partition.source_state_bytes() < 64 * 1024);
    let t = complete_execute(&transcript(), manifest(vec![view.clone()]));
    assert_eq!(t.retention().body_bytes(), 0);
    assert!(t.retained_storage_bytes().unwrap() < 1024 * 1024);
    let foreign = complete_execute(
        &transcript(),
        manifest(vec![
            RepositoryWorkspaceReadView::from_source(make([1; 32])).unwrap(),
        ]),
    );
    let mut source_calls = 0;
    assert_eq!(
        t.try_eq_with_states(&foreign, &mut |a, b| {
            if let (
                RepositoryWorkspaceReadViewStateRef::Source(a),
                RepositoryWorkspaceReadViewStateRef::Source(b),
            ) = (a, b)
            {
                source_calls += 1;
                assert!(!Arc::ptr_eq(a, b));
            }
            Ok::<_, ()>(a == b)
        }),
        Ok(true)
    );
    assert_eq!(source_calls, 1);
    let changed = complete_execute(
        &transcript(),
        manifest(vec![
            RepositoryWorkspaceReadView::from_source(make([9; 32])).unwrap(),
        ]),
    );
    assert_ne!(t, changed);
    assert_eq!(
        t.try_eq_with_states(&changed, &mut |a, b| Ok::<_, ()>(a == b)),
        Ok(false)
    );
    assert!(
        RepositoryWorkspaceReadViewManifest::new(&reservation_with(8, 11, limits()), vec![view])
            .is_err()
    );
}
