//! The actor authenticates the complete current epoch separately from the
//! operation prefix. Available Sources never enlarge that prefix.
use slug_bzlmod_v2::RepositorySourceSnapshotEpoch;
use slug_bzlmod_v2::RepositoryWorkspaceOperation as Op;
use slug_bzlmod_v2::RepositoryWorkspaceReadView as View;
use slug_bzlmod_v2::RepositoryWorkspaceResult as ResultValue;
use slug_bzlmod_v2::RepositoryWorkspaceTranscript;
use slug_workspace_v2::NormalizedAbsolutePath;

use super::*;
use crate::runtime::native_source_access::new_reader_resources;

#[test]
#[ignore = "actual CLI + actor exact Source prefix and equal foreign predecessor; finite native supervisor required"]
fn native_source_link_actor_authenticates_full_epoch_without_granting_undeclared_sources() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(workspace.path().join("payload"), b"source A").unwrap();
    let local = workspace.path().join("local-c");
    std::fs::create_dir(&local).unwrap();
    std::fs::write(local.join("payload"), b"source C").unwrap();
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let readers = new_reader_resources().unwrap();
    let reader_baseline = readers.snapshot().retained_bytes;
    let normalized = NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let scope = scope(&normalized, &resources);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let actor = RepositoryMaterializer::new(normalized.clone());
    let (token, command, a) = initialize(
        &actor,
        workspace.path(),
        &resources,
        &scope,
        &runtime,
        false,
    )
    .unwrap();
    let lease = actor.workspace_lease(token).unwrap();
    let request_c =
        crate::runtime::dice::local_source_request_for_test(workspace.path(), lease, &local);
    let ticket_c = actor
        .reserve_source_snapshot(token, request_c.clone(), &scope)
        .unwrap();
    let input = SourceRootInput::capture(
        request_c,
        &resources,
        &runtime,
        &crate::runtime::ArchiveDistdirPolicy::default(),
        &scope,
        &|| true,
    )
    .unwrap();
    let (command, receipt_c) = runtime
        .block_on(command.initialize_source_snapshot(
            ticket_c.reservation().clone(),
            ticket_c.claim().clone(),
            input,
            &scope,
            &|| true,
        ))
        .unwrap();
    let c = receipt_c.root().clone();
    actor
        .adopt_source_snapshot(token, &ticket_c, receipt_c, &scope)
        .unwrap();
    let epoch = RepositorySourceSnapshotEpoch::fresh(normalized, lease)
        .unwrap()
        .with_snapshot(a.state().clone())
        .unwrap()
        .with_snapshot(c.state().clone())
        .unwrap();
    let results = actor.selected_epoch(token, &[]).unwrap();
    let (command, reservation) =
        super::source_links::general(&actor, token, command, &resources, &scope, &runtime);
    let target_a =
        NormalizedAbsolutePath::new(a.reservation().display_root().as_path().join("payload"))
            .unwrap();
    let operation = Op::Symlink {
        path: "alias".into(),
        target: target_a,
        source: Some(View::from_source(a.state().clone()).unwrap()),
    };
    let request = command
        .request(&reservation, operation.clone(), &scope, &|| true)
        .unwrap();
    let command = actor
        .apply_workspace_with_sources(
            token,
            command,
            request,
            &[],
            &results,
            std::slice::from_ref(a.state()),
            &epoch,
            &readers,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
    let native = &command.epoch().transcripts().unwrap()[0];
    let foreign_source =
        Arc::new(State::new(a.reservation().clone(), a.state().entries().to_vec()).unwrap());
    assert_eq!(foreign_source.as_ref(), a.state().as_ref());
    assert!(!Arc::ptr_eq(&foreign_source, a.state()));
    let mut foreign = RepositoryWorkspaceTranscript::new(
        reservation.clone(),
        native.initial().entry("").unwrap().mode(),
    )
    .unwrap();
    let foreign_op = match operation {
        Op::Symlink { path, target, .. } => Op::Symlink {
            path,
            target,
            source: Some(View::from_source(foreign_source.clone()).unwrap()),
        },
        _ => unreachable!(),
    };
    let foreign_request = foreign.request(foreign_op).unwrap();
    foreign = foreign
        .complete(
            foreign_request,
            ResultValue::Unit,
            native.current().mode(),
            native.current().entries().to_vec(),
        )
        .unwrap();
    assert_eq!(foreign.current().as_ref(), native.current().as_ref());
    assert!(!Arc::ptr_eq(foreign.current(), native.current()));
    let request = foreign.request(Op::Exists("alias".into())).unwrap();
    let command = actor
        .apply_workspace_with_sources(
            token,
            command,
            request,
            &[],
            &results,
            std::slice::from_ref(&foreign_source),
            &epoch,
            &readers,
            &runtime,
            &crate::runtime::ArchiveDistdirPolicy::default(),
            &scope,
            &|| true,
        )
        .unwrap();
    assert!(matches!(
        command.epoch().transcripts().unwrap()[0]
            .records()
            .last()
            .unwrap()
            .result(),
        ResultValue::Exists(true)
    ));
    assert!(
        Arc::ptr_eq(
            command.epoch().transcripts().unwrap()[0]
                .current()
                .source_bindings()[0]
                .source_state()
                .unwrap(),
            a.state()
        ),
        "native history must retain covered canonical Source allocation"
    );
    let request = command
        .request(
            &reservation,
            Op::Symlink {
                path: "undeclared".into(),
                target: NormalizedAbsolutePath::new(
                    c.reservation().display_root().as_path().join("payload"),
                )
                .unwrap(),
                source: Some(View::from_source(c.state().clone()).unwrap()),
            },
            &scope,
            &|| true,
        )
        .unwrap();
    let before = scope.snapshot();
    let observer = command.retirement_observer();
    assert!(
        actor
            .apply_workspace_with_sources(
                token,
                command,
                request,
                &[],
                &results,
                std::slice::from_ref(a.state()),
                &epoch,
                &readers,
                &runtime,
                &crate::runtime::ArchiveDistdirPolicy::default(),
                &scope,
                &|| true
            )
            .is_err(),
        "complete current epoch includes C but exact operation prefix declares only A"
    );
    assert_eq!(scope.snapshot().source_attempts, before.source_attempts);
    assert_eq!(scope.snapshot().decoded_bytes, before.decoded_bytes);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    drop(foreign);
    drop(foreign_source);
    drop(epoch);
    drop(results);
    drop(ticket_c);
    actor.discard(token).unwrap();
    drop(actor);
    drop(a);
    drop(c);
    drop(scope);
    assert_eq!(
        (
            readers.snapshot().native_fds,
            readers.snapshot().retained_bytes
        ),
        (0, reader_baseline)
    );
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
}

#[test]
#[ignore = "actual CLI + deterministic prelaunch current-association drift; finite native supervisor required"]
fn native_source_link_execute_rechecks_current_association_before_child_or_root_mutation() {
    let workspace = tempfile::tempdir().unwrap();
    let local_a = workspace.path().join("local-a");
    let local_b = workspace.path().join("local-b");
    for path in [&local_a, &local_b] {
        std::fs::create_dir(path).unwrap();
        std::fs::write(path.join("payload"), b"body").unwrap();
    }
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let readers = new_reader_resources().unwrap();
    let reader_baseline = readers.snapshot().retained_bytes;
    let normalized = NormalizedAbsolutePath::new(workspace.path()).unwrap();
    let scope = scope(&normalized, &resources);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let actor = RepositoryMaterializer::new(normalized.clone());
    let (token, command, source) = initialize_requested(
        &actor,
        workspace.path(),
        &resources,
        &scope,
        &runtime,
        false,
        |lease| {
            crate::runtime::dice::local_source_request_for_test(workspace.path(), lease, &local_a)
        },
    )
    .unwrap();
    let epoch =
        RepositorySourceSnapshotEpoch::fresh(normalized, actor.workspace_lease(token).unwrap())
            .unwrap()
            .with_snapshot(source.state().clone())
            .unwrap();
    let results = actor.selected_epoch(token, &[]).unwrap();
    let (command, reservation) =
        super::source_links::general(&actor, token, command, &resources, &scope, &runtime);
    let stage = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let command = command.with_process_probe_for_test(stage.clone(), false);
    let manifest = slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest::new(
        &reservation,
        vec![View::from_source(source.state().clone()).unwrap()],
    )
    .unwrap();
    let execute = slug_bzlmod_v2::RepositoryWorkspaceExecute::new(
        ["/bin/sh", "-c", "printf launched > forbidden"]
            .iter()
            .map(|arg| slug_bzlmod_v2::RepositoryWorkspaceArgument::String((*arg).into()))
            .collect::<Vec<_>>()
            .into(),
        slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
        starlark_map::small_map::SmallMap::new(),
        10,
        true,
        slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
            reservation.display_root().clone(),
            PathObservationNamespace::Materialization(reservation.instance()),
        )
        .unwrap(),
        manifest,
    )
    .unwrap();
    let request = command
        .request(&reservation, Op::Execute(execute), &scope, &|| true)
        .unwrap();
    let mut changed = source
        .reservation()
        .request()
        .materialization_request()
        .unwrap()
        .as_ref()
        .clone();
    changed.kind = slug_bzlmod_v2::RepositoryMaterializationKind::Local {
        logical_root: NormalizedAbsolutePath::new(&local_b).unwrap(),
    };
    let mut attributes = changed.repo_spec.attributes.as_ref().clone();
    attributes.insert(
        "path".into(),
        slug_bzlmod_v2::OverrideAttributeValue::String("local-b".into()),
    );
    changed.repo_spec.attributes = Arc::new(attributes);
    let before = resources.snapshot();
    let counters = scope.snapshot();
    let observer = command.retirement_observer();
    let invoked = std::cell::Cell::new(false);
    let result = runtime.block_on(
        actor.execute_workspace_with_current_sources_before_launch_for_test(
            token,
            command,
            request,
            &[],
            &results,
            std::slice::from_ref(source.state()),
            &epoch,
            &readers,
            &scope,
            &|| true,
            || {
                invoked.set(true);
                actor
                    .materialize_native(
                        token,
                        Arc::new(changed),
                        slug_bzlmod_v2::RepositoryMaterializationGeneration(1),
                    )
                    .unwrap();
            },
        ),
    );
    assert!(
        invoked.get(),
        "drift occurs after both authenticated snapshots exist"
    );
    assert!(matches!(result, Err(PhysicalRootError::Reservation)));
    assert_eq!(
        stage.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "no payload launcher stage"
    );
    assert_eq!(
        resources.snapshot().peak_backing_bytes,
        before.peak_backing_bytes,
        "no Process transition or fresh root setup"
    );
    assert_eq!(scope.snapshot().source_attempts, counters.source_attempts);
    assert_eq!(scope.snapshot().decoded_bytes, counters.decoded_bytes);
    assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
    drop(observer);
    drop(epoch);
    drop(results);
    actor.discard(token).unwrap();
    drop(actor);
    drop(source);
    drop(scope);
    assert_eq!(
        (
            readers.snapshot().native_fds,
            readers.snapshot().retained_bytes
        ),
        (0, reader_baseline)
    );
    assert_eq!(
        (
            resources.snapshot().native_fds,
            resources.snapshot().backing_bytes,
            resources.snapshot().retained_bytes
        ),
        (0, 0, baseline)
    );
}

#[test]
#[ignore = "actual CLI + General-linked Source manifest admission; finite native supervisor required"]
fn native_source_link_selected_general_view_requires_its_source_independently_in_manifest() {
    for independently_declared in [false, true] {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("payload"), b"source bytes").unwrap();
        let normalized = NormalizedAbsolutePath::new(workspace.path()).unwrap();
        let resources = bank();
        let baseline = resources.snapshot().retained_bytes;
        let readers = new_reader_resources().unwrap();
        let reader_baseline = readers.snapshot().retained_bytes;
        let scope = scope(&normalized, &resources);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let actor = RepositoryMaterializer::new(normalized.clone());
        let (token, command, a) = initialize(
            &actor,
            workspace.path(),
            &resources,
            &scope,
            &runtime,
            false,
        )
        .unwrap();
        let epoch =
            RepositorySourceSnapshotEpoch::fresh(normalized, actor.workspace_lease(token).unwrap())
                .unwrap()
                .with_snapshot(a.state().clone())
                .unwrap();
        let (command, g_ticket) = super::source_links::general_ticket(
            &actor,
            token,
            command,
            &resources,
            &scope,
            &runtime,
            "linked_general_view",
        );
        let results = actor.selected_epoch(token, &[]).unwrap();
        let operation = Op::Symlink {
            path: "alias".into(),
            target: NormalizedAbsolutePath::new(
                a.reservation().display_root().as_path().join("payload"),
            )
            .unwrap(),
            source: Some(View::from_source(a.state().clone()).unwrap()),
        };
        let request = command
            .request(g_ticket.reservation(), operation, &scope, &|| true)
            .unwrap();
        let command = actor
            .apply_workspace_with_sources(
                token,
                command,
                request,
                &[],
                &results,
                std::slice::from_ref(a.state()),
                &epoch,
                &readers,
                &runtime,
                &crate::runtime::ArchiveDistdirPolicy::default(),
                &scope,
                &|| true,
            )
            .unwrap();
        let transcript = &command.epoch().transcripts().unwrap()[0];
        let realization = transcript.realize(transcript.records().len()).unwrap();
        let g_request = Arc::new(slug_bzlmod_v2::RepositoryMaterializationRequest {
            id: slug_bzlmod_v2::RepositoryMaterializationRequestId {
                workspace: actor.workspace().clone(),
                canonical_repo: g_ticket.reservation().repository().clone(),
            },
            repo_spec: g_ticket.repo_spec().as_ref().clone(),
            kind: slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(realization),
        });
        let (command, sealed) = runtime
            .block_on(command.finalize_workspace(g_request.clone(), &scope, &|| true))
            .unwrap();
        actor
            .adopt_workspace(
                token,
                &g_ticket,
                slug_bzlmod_v2::RepositoryMaterializationGeneration(1),
                sealed,
                &scope,
            )
            .unwrap();
        let (command, b_ticket) = super::source_links::general_ticket(
            &actor,
            token,
            command,
            &resources,
            &scope,
            &runtime,
            "view_executor",
        );
        let slug_bzlmod_v2::RepositoryMaterializationKind::GeneralWorkspace(realization) =
            &g_request.kind
        else {
            unreachable!()
        };
        let mut views = vec![
            View::new(
                g_request.repo_spec.clone(),
                realization.final_state().clone(),
            )
            .unwrap(),
        ];
        let declared = if independently_declared {
            views.push(View::from_source(a.state().clone()).unwrap());
            vec![a.state().clone()]
        } else {
            vec![]
        };
        let manifest =
            slug_bzlmod_v2::RepositoryWorkspaceReadViewManifest::new(b_ticket.reservation(), views)
                .unwrap();
        let alias = g_ticket
            .reservation()
            .display_root()
            .as_path()
            .join("alias");
        let execute = slug_bzlmod_v2::RepositoryWorkspaceExecute::new(
            [
                "/bin/sh",
                "-c",
                "/bin/cat \"$1\" > out",
                "proof",
                alias.to_str().unwrap(),
            ]
            .iter()
            .map(|arg| slug_bzlmod_v2::RepositoryWorkspaceArgument::String((*arg).into()))
            .collect::<Vec<_>>()
            .into(),
            slug_bzlmod_v2::RepositoryEnvironmentSnapshot::empty(),
            starlark_map::small_map::SmallMap::new(),
            10,
            true,
            slug_bzlmod_v2::HostRepositoryLabelPathValue::new(
                b_ticket.reservation().display_root().clone(),
                PathObservationNamespace::Materialization(b_ticket.reservation().instance()),
            )
            .unwrap(),
            manifest,
        )
        .unwrap();
        let request = command
            .request(
                b_ticket.reservation(),
                Op::Execute(execute),
                &scope,
                &|| true,
            )
            .unwrap();
        let selected = [g_request.clone()];
        let results = actor.selected_epoch(token, &selected).unwrap();
        let stage = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let command = command.with_process_probe_for_test(stage.clone(), false);
        let observer = command.retirement_observer();
        let before = resources.snapshot();
        let counters = scope.snapshot();
        let result = runtime.block_on(actor.execute_workspace_with_current_sources(
            token,
            command,
            request,
            &selected,
            &results,
            &declared,
            &epoch,
            &readers,
            &scope,
            &|| true,
        ));
        if independently_declared {
            let command = result.unwrap();
            let transcript = command
                .epoch()
                .transcripts()
                .unwrap()
                .iter()
                .find(|t| t.reservation() == b_ticket.reservation())
                .unwrap();
            assert!(
                matches!(transcript.records().last().unwrap().result(), ResultValue::Execute(result) if result.return_code() == 0)
            );
            assert!(
                matches!(transcript.current().entry("out").unwrap().node(), slug_bzlmod_v2::RepositoryWorkspaceNode::File(body) if body.as_ref() == b"source bytes")
            );
            drop(command);
        } else {
            assert!(matches!(
                result,
                Err(PhysicalRootError::Reservation | PhysicalRootError::UnsupportedOperation)
            ));
            assert_eq!(
                stage.load(std::sync::atomic::Ordering::SeqCst),
                0,
                "available retained A must not silently enter a G-only manifest"
            );
            assert_eq!(
                resources.snapshot().peak_backing_bytes,
                before.peak_backing_bytes
            );
            assert_eq!(scope.snapshot().source_attempts, counters.source_attempts);
            assert_eq!(scope.snapshot().decoded_bytes, counters.decoded_bytes);
        }
        assert!(observer.wait_until(Instant::now() + Duration::from_secs(1)));
        drop(observer);
        drop(results);
        drop(selected);
        drop(g_request);
        drop(declared);
        drop(epoch);
        drop(b_ticket);
        drop(g_ticket);
        actor.discard(token).unwrap();
        drop(actor);
        drop(a);
        drop(scope);
        assert_eq!(
            (
                readers.snapshot().native_fds,
                readers.snapshot().retained_bytes
            ),
            (0, reader_baseline)
        );
        assert_eq!(
            (
                resources.snapshot().native_fds,
                resources.snapshot().backing_bytes,
                resources.snapshot().retained_bytes
            ),
            (0, 0, baseline)
        );
    }
}
