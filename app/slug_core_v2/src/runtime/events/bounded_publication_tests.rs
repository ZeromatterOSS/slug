use std::sync::Arc;

use slug_events_v2::EvaluationDiagnosticLevel;
use slug_events_v2::EvaluationEvent;
use slug_events_v2::EventBatch;
use slug_events_v2::StarlarkSourceLocation;

use super::AcceptedCommand;
use super::CommandOutputBuffer;
use super::TerminalOutput;

#[test]
fn bounded_publication_preserves_exact_mixed_stderr_and_refuses_one_under() {
    let identity = Arc::new(7);
    let batches: Arc<[EventBatch]> = Arc::from([
        EventBatch::from_events([
            EvaluationEvent::StarlarkPrint {
                location: StarlarkSourceLocation::new(Arc::from("selected.bzl"), 2, 5),
                text: "already terminated\n".into(),
            },
            EvaluationEvent::Diagnostic {
                level: EvaluationDiagnosticLevel::Warning,
                text: "add a newline".into(),
            },
        ]),
        EventBatch::from_events([EvaluationEvent::RepositoryExecuteOutput {
            command_lease: 9,
            operation_index: 2,
            stdout: Arc::from([0xff, 0]),
            stderr: Arc::from([0xfe, 1]),
        }]),
    ]);
    let project = || {
        AcceptedCommand::new(
            identity.clone(),
            CommandOutputBuffer {
                batches: Arc::clone(&batches),
            },
        )
        .project(|_| TerminalOutput::new(0, "row\n".into(), "formatter message\n".into()))
    };

    let (_, _, _, ordinary_stderr) = project().publish().into_parts();
    let newline: &[u8] = if cfg!(windows) { b"\r\n" } else { b"\n" };
    let expected = [
        b"DEBUG: selected.bzl:2:5: already terminated\nWARNING: add a newline".as_slice(),
        newline,
        &[0xff, 0, 0xfe, 1],
        b"formatter message\n",
    ]
    .concat();
    assert_eq!(ordinary_stderr, expected);

    let (terminal, code, stdout, stderr) = project()
        .publish_with_stderr_limit(expected.len())
        .into_parts();
    assert!(Arc::ptr_eq(&terminal, &identity));
    assert_eq!(code, 0);
    assert_eq!(stdout, "row\n");
    assert_eq!(stderr, expected);

    let (terminal, code, stdout, stderr) = project()
        .publish_with_stderr_limit(expected.len() - 1)
        .into_parts();
    assert!(Arc::ptr_eq(&terminal, &identity));
    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert_eq!(
        stderr.as_slice(),
        b"ERROR: command stderr byte limit exceeded\n"
    );
}

#[test]
fn bounded_publication_preserves_setup_failure_and_independent_stdout() {
    let batches: Arc<[EventBatch]> =
        Arc::from([EventBatch::from_events([EvaluationEvent::Diagnostic {
            level: EvaluationDiagnosticLevel::Error,
            text: "accepted event\n".into(),
        }])]);
    let setup_failure = AcceptedCommand::new(
        (),
        CommandOutputBuffer {
            batches: Arc::clone(&batches),
        },
    )
    .project(|_| TerminalOutput::new(1, String::new(), "invalid formatter\n".into()))
    .publish_with_stderr_limit(b"ERROR: accepted event\ninvalid formatter\n".len())
    .into_parts();
    assert_eq!(setup_failure.1, 1);
    assert!(setup_failure.2.is_empty());
    assert_eq!(
        setup_failure.3.as_slice(),
        b"ERROR: accepted event\ninvalid formatter\n"
    );

    let stdout = "an independent stdout row larger than the stderr limit\n";
    let admitted = AcceptedCommand::new((), CommandOutputBuffer { batches })
        .project(|_| TerminalOutput::new(0, stdout.into(), String::new()))
        .publish_with_stderr_limit(b"ERROR: accepted event\n".len())
        .into_parts();
    assert_eq!(admitted.1, 0);
    assert_eq!(admitted.2, stdout);
    assert_eq!(admitted.3.as_slice(), b"ERROR: accepted event\n");

    let terminal_only = AcceptedCommand::new(
        (),
        CommandOutputBuffer {
            batches: Arc::from([]),
        },
    )
    .project(|_| TerminalOutput::new(1, String::new(), "invalid formatter\n".into()))
    .publish_with_stderr_limit(0)
    .into_parts();
    assert_eq!(terminal_only.1, 2);
    assert!(terminal_only.2.is_empty());
    assert!(terminal_only.3.is_empty());
}
