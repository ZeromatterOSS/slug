//! Genuine authored General repository glob through the ordinary cquery graph.
use slug_bzlmod_v2::RepositoryWorkspaceNode;
use slug_bzlmod_v2::RepositoryWorkspaceOperation;
use slug_workspace_v2::PathObservationNamespace;
use slug_workspace_v2::PathObservationOperation;

use super::*;

#[test]
#[ignore = "actual compiled CLI + original General repository glob; finite native supervisor and SLUG_V2_BIN required"]
fn native_original_general_repository_glob_retains_anchor_and_retires_owners() {
    const BUILD: &str = "filegroup(name='collected', srcs=glob(['**/*.txt'], exclude=['**/skip.txt']), visibility=['//visibility:public'])\n";
    let source = format!(
        r#"
def _repository(ctx):
    ctx.file("top.txt", "top\n", executable=False)
    ctx.file("sub/deep.txt", "deep\n", executable=False)
    ctx.file("sub/skip.txt", "excluded\n", executable=False)
    ctx.file("BUILD.bazel", {build:?}, executable=False)
repository = repository_rule(implementation=_repository)
def _extension(ctx):
    repository(name="proof")
ext = module_extension(implementation=_extension)
"#,
        build = BUILD,
    );
    let (workspace, policy) = original_workspace(&source, "collected");
    let resources = bank();
    let baseline = resources.snapshot().retained_bytes;
    let mut runtime =
        WorkspaceRuntime::new(workspace.path(), crate::runtime::ProcessHostOwner::native())
            .unwrap();
    assert!(runtime.source_scope().is_none());
    assert_eq!(
        runtime.repository_workspace_policy(),
        slug_bzlmod_v2::RepositoryWorkspaceExecutionPolicy::Disabled
    );
    runtime
        .enable_general_repository_workspaces_for_test(resources.clone(), limits())
        .unwrap();

    // The original repository_rule creates every real file and native root.
    // No request, materialization result, State, lease or path epoch is injected.
    let accepted = runtime
        .cquery_command_with_bzlmod_inputs(
            "deps(//:use)",
            false,
            true,
            policy,
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[],
            CommandConfigurationOverlay::new(Arc::from([])),
        )
        .unwrap_or_else(|_| panic!("original General repository glob must complete"));
    let terminal = accepted.terminal_for_test();
    assert!(
        terminal.as_ref().is_ok(),
        "ordinary configured query must succeed"
    );
    let labels = terminal.as_ref().as_ref().unwrap().starlark_label_stdout();
    assert!(labels.lines().any(|label| label.ends_with("//:top.txt")));
    assert!(
        labels
            .lines()
            .any(|label| label.ends_with("//:sub/deep.txt"))
    );
    assert!(
        !labels
            .lines()
            .any(|label| label.ends_with("//:sub/skip.txt"))
    );

    let state = runtime.native_demand_sessions.state.lock().unwrap();
    assert!(matches!(state.phase, NativeDemandLeasePhase::Idle));
    let requests = state.accepted.selected.repository_requests();
    let general = requests
        .iter()
        .filter_map(|request| match &request.kind {
            RepositoryMaterializationKind::GeneralWorkspace(realization) => Some(realization),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        general.len(),
        1,
        "one actual authored repository invocation"
    );
    let realization = general[0];
    let reservation = realization.reservation();
    let namespace = PathObservationNamespace::Materialization(reservation.instance());
    let root = reservation.display_root();
    let anchor = realization.final_state().resolution_anchor().unwrap();
    assert_eq!(anchor.namespace(), namespace);
    assert_eq!(anchor.root(), root);
    assert!(
        !root.as_path().exists(),
        "native virtual root never appears on Host"
    );
    let files = realization
        .transcript()
        .records()
        .iter()
        .filter(|record| {
            matches!(
                record.request().operation(),
                RepositoryWorkspaceOperation::File { .. }
            )
        })
        .count();
    assert_eq!(
        files, 4,
        "actual File records arise from the four original calls"
    );
    assert!(realization.transcript().records().iter().all(|record| {
        !matches!(
            record.request().operation(),
            RepositoryWorkspaceOperation::Execute(_) | RepositoryWorkspaceOperation::Archive(_)
        )
    }));
    for (path, expected) in [
        ("top.txt", b"top\n".as_slice()),
        ("sub/deep.txt", b"deep\n".as_slice()),
        ("sub/skip.txt", b"excluded\n".as_slice()),
        ("BUILD.bazel", BUILD.as_bytes()),
    ] {
        let RepositoryWorkspaceNode::File(body) =
            realization.final_state().entry(path).unwrap().node()
        else {
            panic!("actual native snapshot must retain the authored regular file")
        };
        assert!(body.equals_slice(expected));
    }

    let observations = state.accepted.path_observations.observations();
    let own = observations
        .keys()
        .filter(|demand| demand.namespace() == namespace)
        .collect::<Vec<_>>();
    assert!(!own.is_empty());
    assert!(
        own.iter()
            .all(|demand| demand.path().as_path().starts_with(root.as_path())),
        "every own-incarnation observation remains beneath the exact native root"
    );
    assert!(
        own.iter().any(
            |demand| demand.operation() == PathObservationOperation::DirectoryEntries
                && demand.path() == root
        ),
        "glob observes the actual root listing"
    );
    assert!(
        own.iter().any(
            |demand| demand.operation() == PathObservationOperation::DirectoryEntries
                && demand.path().as_path() == root.as_path().join("sub")
        ),
        "recursive glob observes the actual nested directory"
    );

    let weak_state = Arc::downgrade(realization.final_state());
    let weak_reservation = Arc::downgrade(reservation);
    let RepositoryWorkspaceNode::File(body) =
        realization.final_state().entry("top.txt").unwrap().node()
    else {
        unreachable!()
    };
    let weak_body = body.downgrade();
    assert!(weak_state.upgrade().is_some());
    assert!(weak_body.upgrade().is_some());
    drop(anchor);
    drop(state);
    drop(accepted);
    drop(runtime);
    // As in external_bzl_frontier_tests, the dedicated DICE graph actor
    // retires historical values after its closed channel has drained.
    let retirement_deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while (weak_state.strong_count() != 0
        || weak_reservation.strong_count() != 0
        || weak_body.strong_count() != 0
        || {
            let snapshot = resources.snapshot();
            snapshot.retained_bytes != baseline
                || snapshot.native_fds != 0
                || snapshot.backing_bytes != 0
        })
        && std::time::Instant::now() < retirement_deadline
    {
        std::thread::yield_now();
    }
    assert!(weak_state.upgrade().is_none());
    assert!(weak_reservation.upgrade().is_none());
    assert!(weak_body.upgrade().is_none());
    drop(weak_state);
    drop(weak_reservation);
    drop(weak_body);
    let after = resources.snapshot();
    assert_eq!(after.retained_bytes, baseline);
    assert_eq!(after.native_fds, 0);
    assert_eq!(after.backing_bytes, 0);
}
