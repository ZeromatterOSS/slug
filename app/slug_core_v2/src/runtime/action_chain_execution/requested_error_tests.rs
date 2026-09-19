//! Requested build failures retain the ordinary native error and event owners.
use super::*;
use crate::runtime::TerminalOutput;

fn projection(error: Option<&BuildCommandError>) -> TerminalOutput {
    match error {
        Some(error) => {
            let (kind, code) = error.terminal_error();
            TerminalOutput::new(code, String::new(), format!("{kind}: {error}\n"))
        }
        None => TerminalOutput::new(0, String::new(), "repaired\n".into()),
    }
}

fn execute(
    workspace: &Workspace,
    targets: &[TargetPattern],
    transport: &Fake,
    publish: bool,
) -> Result<
    AcceptedCommand<Result<RequestedActionResult<Output>, BuildCommandError>>,
    BuildCommandError,
> {
    let policy = BzlmodCommandPolicyKey::from_flags(None, false).unwrap();
    let environment = BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap();
    let registries = [format!(
        "file://{}/empty-registry",
        workspace.root.path().display()
    )];
    if publish {
        workspace
            .runtime
            .execute_and_publish_requested_actions_with_repository_environment(
                targets,
                policy,
                environment,
                LockfileMode::Update,
                &registries,
                Default::default(),
                Default::default(),
                transport,
            )
    } else {
        workspace
            .runtime
            .execute_requested_actions_with_repository_environment(
                targets,
                policy,
                environment,
                LockfileMode::Update,
                &registries,
                Default::default(),
                Default::default(),
                transport,
            )
    }
}

fn error_fixture(analysis_error: bool) -> (Workspace, String, String, [TargetPattern; 2]) {
    let workspace = Workspace::new();
    let mut good = DEFS.replace(
        "def _impl(ctx):",
        "print('REQUESTED_LOADING')\ndef _impl(ctx):\n    print('REQUESTED_ANALYSIS')",
    );
    let targets = [
        TargetPattern::parse("//:one").unwrap(),
        TargetPattern::parse(if analysis_error {
            "//:one"
        } else {
            "//:conflict"
        })
        .unwrap(),
    ];
    let broken = if analysis_error {
        good.replace(
            "    seed =",
            "    fail('REQUESTED_ANALYSIS_FAILURE')\n    seed =",
        )
    } else {
        good.push_str("\ndef _conflict(ctx):\n    print('REQUESTED_CONFLICT')\n    out = ctx.actions.declare_file('seed')\n    ctx.actions.write(out, 'seed')\n    return [DefaultInfo(files=depset([]))]\nconflict = rule(implementation=_conflict)\n");
        let build = workspace.root.path().join("BUILD.bazel");
        fs::write(
            &build,
            format!(
                "load(':defs.bzl', 'conflict')\n{}\nconflict(name='conflict')\n",
                fs::read_to_string(&build).unwrap()
            ),
        )
        .unwrap();
        good.replace(
            "    ctx.actions.write(out, 'seed')",
            "    ctx.actions.write(out, 'conflicting bytes')",
        )
    };
    (workspace, good, broken, targets)
}

fn error_repair_error(analysis_error: bool, publish: bool) {
    let (workspace, good, broken, targets) = error_fixture(analysis_error);
    let transport = Fake::new(workspace.root.path(), Behavior::Normal);
    let baseline = WorkspaceRuntime::new(
        workspace.root.path(),
        crate::runtime::ProcessHostOwner::native(),
    )
    .unwrap();
    let mut first_error = None;
    for (iteration, definitions) in [&broken, &good, &broken].into_iter().enumerate() {
        fs::write(workspace.root.path().join("defs.bzl"), definitions).unwrap();
        let accepted = execute(&workspace, &targets, &transport, publish)
            .unwrap_or_else(|error| panic!("requested build iteration {iteration}: {error}"));
        let baseline_accepted = baseline
            .build_command_with_repository_environment(
                &targets,
                BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
                BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
                LockfileMode::Update,
                &[format!(
                    "file://{}/empty-registry",
                    workspace.root.path().display()
                )],
                Default::default(),
                Default::default(),
            )
            .unwrap_or_else(|error| panic!("ordinary build iteration {iteration}: {error}"));
        let expected_error = baseline_accepted
            .terminal_for_test()
            .as_ref()
            .as_ref()
            .err()
            .cloned();
        let (_, expected_code, expected_stdout, expected_stderr) = baseline_accepted
            .project(|result| projection(result.as_ref().as_ref().err()))
            .publish()
            .into_parts();
        let result = accepted.terminal_for_test();
        if iteration == 1 {
            let result = result.as_ref().unwrap();
            assert!(expected_error.is_none());
            assert_eq!(result.output().unwrap().executed, [0, 1, 2, 3, 4]);
            assert_eq!(result.published_outputs().len(), usize::from(publish));
            assert!(result.inputs().evaluation().is_ok());
        } else {
            let actual = result.as_ref().err().expect("typed accepted build error");
            assert_eq!(Some(actual), expected_error.as_ref());
            assert_eq!(actual.is_analysis_error(), analysis_error);
            assert_eq!(
                actual.terminal_error(),
                (
                    if analysis_error {
                        "build_runtime_error"
                    } else {
                        "configured_action_conflict"
                    },
                    2
                )
            );
            if analysis_error {
                assert!(matches!(actual.kind, BuildCommandErrorKind::Analysis(_)));
                assert!(actual.to_string().contains("REQUESTED_ANALYSIS_FAILURE"));
            } else {
                assert!(matches!(
                    actual.kind,
                    BuildCommandErrorKind::ActionClosure(_)
                ));
            }
            if iteration == 0 {
                first_error = Some(actual.clone());
                assert!(!workspace.output_path().exists());
            } else {
                assert_eq!(Some(actual), first_error.as_ref());
            }
        }
        let (_, code, stdout, stderr) = accepted
            .project(|result| projection(result.as_ref().err()))
            .publish()
            .into_parts();
        assert_eq!(
            (code, stdout, stderr.clone()),
            (expected_code, expected_stdout, expected_stderr)
        );
        if (analysis_error && iteration == 1) || (!analysis_error && iteration == 0) {
            assert!(
                stderr.contains("REQUESTED_LOADING"),
                "selected loading diagnostic missing: {stderr}"
            );
            assert!(
                stderr.contains("REQUESTED_ANALYSIS"),
                "selected analysis diagnostic missing: {stderr}"
            );
        }
        if !analysis_error && iteration > 0 {
            assert!(
                !stderr.contains("DEBUG:"),
                "warm diagnostics replayed: {stderr}"
            );
            let expected = if iteration == 1 {
                "repaired\n".to_owned()
            } else {
                format!(
                    "configured_action_conflict: {}\n",
                    first_error.as_ref().unwrap()
                )
            };
            assert_eq!(stderr, expected);
        }
        let completed = usize::from(iteration > 0);
        assert_eq!(transport.starts.load(Ordering::SeqCst), completed);
        assert_eq!(transport.stages.load(Ordering::SeqCst), 5 * completed);
        assert_eq!(transport.executions.load(Ordering::SeqCst), 5 * completed);
        assert_eq!(
            transport.output_stages.load(Ordering::SeqCst),
            usize::from(publish) * completed
        );
        let snapshot = workspace.snapshot();
        assert!(snapshot.path_observations.observations().iter().any(|(demand, value)| {
            demand.path().as_path() == workspace.root.path().join("defs.bzl")
                && matches!(value.as_ref(), PathObservationResult::FileBytes(PathOperationResult::Present(bytes)) if bytes.as_ref() == definitions.as_bytes())
        }), "accepted failure/recovery must keep the current build frontier");
        if publish && iteration > 0 {
            assert_eq!(fs::read(workspace.output_path()).unwrap(), b"aaa");
            workspace.assert_stage_cleanup();
        }
    }
}

#[test]
fn requested_analysis_error_terminal_preserves_events_and_recovers() {
    error_repair_error(true, false);
}

#[test]
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn requested_conflict_error_terminal_preserves_category_and_skips_publication() {
    error_repair_error(false, true);
}

#[test]
fn requested_preparation_exposes_failure_without_panicking_and_selected_still_fails() {
    let workspace = Workspace::new();
    fs::write(
        workspace.root.path().join("defs.bzl"),
        DEFS.replace(
            "    seed =",
            "    fail('PREPARE_ANALYSIS_FAILURE')\n    seed =",
        ),
    )
    .unwrap();
    let prepared = workspace
        .runtime
        .prepare_requested_action_inputs_with_repository_environment(
            &[TargetPattern::parse("//:one").unwrap()],
            BzlmodCommandPolicyKey::from_flags(None, false).unwrap(),
            BzlmodEnvironmentPolicyKey::from_bzlmod_allow_yanked_versions(None).unwrap(),
            LockfileMode::Update,
            &[format!(
                "file://{}/empty-registry",
                workspace.root.path().display()
            )],
            Default::default(),
            Default::default(),
        )
        .unwrap();
    let inputs = prepared.terminal_for_test();
    let error = inputs.evaluation().unwrap_err();
    assert!(matches!(error.kind, BuildCommandErrorKind::Analysis(_)));
    assert_eq!(inputs.plan().unwrap_err().as_ref(), error.to_string());
    assert_eq!(inputs.sources().len(), 0);
    assert!(!inputs.observations().observations().is_empty());
    let transport = Fake::new(workspace.root.path(), Behavior::Normal);
    let selected = workspace.run(4, &transport).unwrap_err();
    assert!(selected.to_string().contains(&error.to_string()));
    assert_eq!(transport.starts.load(Ordering::SeqCst), 0);
    assert_eq!(transport.output_stages.load(Ordering::SeqCst), 0);
    assert!(!workspace.output_path().exists());
}
