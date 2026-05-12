#![allow(deprecated)] // BEP retains deprecated fields; tests assert on them.

//! Table-driven tests for the `BuckEvent` → BEP translation layer.
//!
//! Each case provides a Slug event and asserts which BEP event(s) the
//! translator emits. Lossy conversions are checked by asserting the
//! fields that BuildBuddy / jq / `bazel --build_event_json_file` consumers
//! actually read.

use slug_build_event_stream::build_event_stream as bep;
use slug_build_event_stream::translate::BuildEventContext;
use slug_build_event_stream::translate::make_aborted;
use slug_build_event_stream::translate::make_progress;
use slug_build_event_stream::translate::translate_buck_event;
use slug_data as data;
use prost_types::Timestamp;

fn ctx() -> BuildEventContext {
    BuildEventContext {
        invocation_id: "11111111-1111-4111-8111-111111111111".to_owned(),
        build_tool_version: "slug-test".to_owned(),
        root_cell_name: "ws".to_owned(),
        workspace_directory: "/tmp/ws".to_owned(),
        working_directory: "/tmp/ws".to_owned(),
        user: "tester".to_owned(),
        host: "localhost".to_owned(),
        command: "build".to_owned(),
        cli_args: vec!["//...".to_owned()],
        server_pid: 123,
    }
}

fn make_event(data: data::buck_event::Data) -> data::BuckEvent {
    data::BuckEvent {
        timestamp: Some(Timestamp {
            seconds: 1_700_000_000,
            nanos: 0,
        }),
        trace_id: "11111111-1111-4111-8111-111111111111".to_owned(),
        span_id: 1,
        parent_id: 0,
        data: Some(data),
    }
}

#[test]
fn command_start_becomes_build_started() {
    let event = make_event(data::buck_event::Data::SpanStart(data::SpanStartEvent {
        data: Some(data::span_start_event::Data::Command(data::CommandStart {
            metadata: Default::default(),
            cli_args: vec!["//...".to_owned()],
            tags: Vec::new(),
            data: Some(data::command_start::Data::Build(data::BuildCommandStart {})),
        })),
    }));

    let out = translate_buck_event(&ctx(), &event);
    // CommandStart now expands into the bracket of metadata events Bazel
    // emits at invocation start: Started, BuildMetadata,
    // UnstructuredCommandLine, 3x StructuredCommandLine, OptionsParsed,
    // WorkspaceStatus.
    assert_eq!(out.len(), 8);

    let payload = out[0].payload.as_ref().expect("payload");
    match payload {
        bep::build_event::Payload::Started(started) => {
            assert_eq!(started.uuid, "11111111-1111-4111-8111-111111111111");
            assert_eq!(started.command, "build");
            assert_eq!(started.workspace_directory, "/tmp/ws");
            assert_eq!(started.build_tool_version, "slug-test");
            assert_eq!(started.user, "tester");
            assert_eq!(
                started.start_time.as_ref().map(|t| t.seconds),
                Some(1_700_000_000)
            );
        }
        other => panic!("expected Started, got {other:?}"),
    }

    // Announces all the metadata events + BuildFinished.
    assert!(out[0].children.len() >= 7);

    // Spot-check the other events in the burst.
    let payload_kinds: Vec<&str> = out
        .iter()
        .map(|e| match e.payload.as_ref().expect("payload") {
            bep::build_event::Payload::Started(_) => "Started",
            bep::build_event::Payload::BuildMetadata(_) => "BuildMetadata",
            bep::build_event::Payload::UnstructuredCommandLine(_) => "UnstructuredCommandLine",
            bep::build_event::Payload::StructuredCommandLine(_) => "StructuredCommandLine",
            bep::build_event::Payload::OptionsParsed(_) => "OptionsParsed",
            bep::build_event::Payload::WorkspaceStatus(_) => "WorkspaceStatus",
            _ => "?",
        })
        .collect();
    assert_eq!(
        payload_kinds,
        vec![
            "Started",
            "BuildMetadata",
            "UnstructuredCommandLine",
            "StructuredCommandLine",
            "StructuredCommandLine",
            "StructuredCommandLine",
            "OptionsParsed",
            "WorkspaceStatus",
        ]
    );
}

#[test]
fn command_end_success_becomes_build_finished() {
    let event = make_event(data::buck_event::Data::SpanEnd(data::SpanEndEvent {
        stats: None,
        duration: None,
        data: Some(data::span_end_event::Data::Command(data::CommandEnd {
            is_success: true,
            build_result: Some(data::BuildResult {
                build_completed: true,
            }),
            data: Some(data::command_end::Data::Build(data::BuildCommandEnd {
                unresolved_target_patterns: Vec::new(),
            })),
        })),
    }));

    let out = translate_buck_event(&ctx(), &event);
    assert_eq!(out.len(), 1);
    // `BuildFinished` is no longer terminal — `BuildMetrics` (emitted by
    // the subscriber on finalize) closes the stream now.
    assert!(!out[0].last_message);

    match out[0].payload.as_ref().expect("payload") {
        bep::build_event::Payload::Finished(finished) => {
            assert!(finished.overall_success);
            let code = finished.exit_code.as_ref().expect("exit_code");
            assert_eq!(code.code, 0);
            assert_eq!(code.name, "SUCCESS");
        }
        other => panic!("expected Finished, got {other:?}"),
    }
}

#[test]
fn command_end_failure_marks_build_failure() {
    let event = make_event(data::buck_event::Data::SpanEnd(data::SpanEndEvent {
        stats: None,
        duration: None,
        data: Some(data::span_end_event::Data::Command(data::CommandEnd {
            is_success: false,
            build_result: Some(data::BuildResult {
                build_completed: false,
            }),
            data: Some(data::command_end::Data::Build(data::BuildCommandEnd {
                unresolved_target_patterns: Vec::new(),
            })),
        })),
    }));

    let out = translate_buck_event(&ctx(), &event);
    match out[0].payload.as_ref().expect("payload") {
        bep::build_event::Payload::Finished(f) => {
            assert!(!f.overall_success);
            assert_eq!(f.exit_code.as_ref().expect("exit_code").code, 1);
        }
        _ => panic!("expected Finished"),
    }
}

#[test]
fn analysis_end_becomes_target_configured() {
    let event = make_event(data::buck_event::Data::SpanEnd(data::SpanEndEvent {
        stats: None,
        duration: None,
        data: Some(data::span_end_event::Data::Analysis(data::AnalysisEnd {
            target: Some(data::analysis_end::Target::StandardTarget(
                data::ConfiguredTargetLabel {
                    label: Some(data::TargetLabel {
                        package: "foo/bar".to_owned(),
                        name: "baz".to_owned(),
                    }),
                    configuration: Some(data::Configuration {
                        full_name: "opt#abc123".to_owned(),
                    }),
                    execution_configuration: None,
                },
            )),
            rule: "cc_library".to_owned(),
            profile: None,
            declared_actions: Some(3),
            declared_artifacts: Some(5),
        })),
    }));

    let out = translate_buck_event(&ctx(), &event);
    // AnalysisEnd produces both TargetConfigured and TargetCompleted so
    // BuildBuddy renders the per-target summary cards.
    assert_eq!(out.len(), 2);

    match &out[0].id.as_ref().expect("id").id {
        Some(bep::build_event_id::Id::TargetConfigured(c)) => {
            assert_eq!(c.label, "//foo/bar:baz");
        }
        _ => panic!("expected TargetConfiguredId"),
    }
    match &out[1].id.as_ref().expect("id").id {
        Some(bep::build_event_id::Id::TargetCompleted(c)) => {
            assert_eq!(c.label, "//foo/bar:baz");
        }
        _ => panic!("expected TargetCompletedId"),
    }
    match out[0].payload.as_ref().expect("payload") {
        bep::build_event::Payload::Configured(c) => {
            assert_eq!(c.target_kind, "cc_library");
        }
        _ => panic!("expected Configured"),
    }
}

#[test]
fn parsed_target_patterns_becomes_pattern_expanded() {
    let event = make_event(data::buck_event::Data::Instant(data::InstantEvent {
        data: Some(data::instant_event::Data::TargetPatterns(
            data::ParsedTargetPatterns {
                target_patterns: vec![
                    data::TargetPattern {
                        value: "//foo/...".to_owned(),
                    },
                    data::TargetPattern {
                        value: "//bar:baz".to_owned(),
                    },
                ],
            },
        )),
    }));

    let out = translate_buck_event(&ctx(), &event);
    assert_eq!(out.len(), 1);
    match &out[0].id.as_ref().expect("id").id {
        Some(bep::build_event_id::Id::Pattern(p)) => {
            assert_eq!(
                p.pattern,
                vec!["//foo/...".to_owned(), "//bar:baz".to_owned()]
            );
        }
        _ => panic!("expected PatternExpandedId"),
    }
}

#[test]
fn configuration_created_becomes_configuration() {
    let event = make_event(data::buck_event::Data::Instant(data::InstantEvent {
        data: Some(data::instant_event::Data::ConfigurationCreated(
            data::ConfigurationCreated {
                cfg: Some(data::ConfigurationWithConstraints {
                    full_name: "opt#abc123".to_owned(),
                    constraint: Vec::new(),
                }),
            },
        )),
    }));

    let out = translate_buck_event(&ctx(), &event);
    assert_eq!(out.len(), 1);
    match &out[0].id.as_ref().expect("id").id {
        Some(bep::build_event_id::Id::Configuration(c)) => {
            assert_eq!(c.id, "opt#abc123");
        }
        _ => panic!("expected ConfigurationId"),
    }
}

#[test]
fn unknown_event_emits_nothing() {
    let event = make_event(data::buck_event::Data::SpanStart(data::SpanStartEvent {
        data: Some(data::span_start_event::Data::Fake(data::FakeStart {
            caramba: String::new(),
        })),
    }));

    let out = translate_buck_event(&ctx(), &event);
    assert!(out.is_empty(), "expected empty, got: {out:?}");
}

#[test]
fn make_aborted_carries_reason_and_description() {
    let id = bep::BuildEventId {
        id: Some(bep::build_event_id::Id::BuildFinished(
            bep::build_event_id::BuildFinishedId {},
        )),
    };
    let aborted = make_aborted(id, bep::aborted::AbortReason::UserInterrupted, "Ctrl+C");
    match aborted.payload.as_ref().expect("payload") {
        bep::build_event::Payload::Aborted(a) => {
            assert_eq!(a.reason, bep::aborted::AbortReason::UserInterrupted as i32);
            assert_eq!(a.description, "Ctrl+C");
        }
        _ => panic!("expected Aborted"),
    }
    assert!(aborted.last_message);
}

#[test]
fn action_execution_end_becomes_action_executed() {
    let event = make_event(data::buck_event::Data::SpanEnd(data::SpanEndEvent {
        stats: None,
        duration: Some(prost_types::Duration {
            seconds: 2,
            nanos: 0,
        }),
        data: Some(data::span_end_event::Data::ActionExecution(Box::new(
            data::ActionExecutionEnd {
                key: Some(data::ActionKey {
                    id: b"\x01\x02\x03".to_vec(),
                    key: "deferred-key-1".to_owned(),
                    owner: None,
                }),
                kind: data::ActionKind::Run as i32,
                name: Some(data::ActionName {
                    category: "cxx_compile".to_owned(),
                    identifier: "main.cpp".to_owned(),
                    progress_message: String::new(),
                }),
                failed: false,
                error: None,
                always_print_stderr: false,
                execution_kind: data::ActionExecutionKind::Local as i32,
                wall_time: Some(prost_types::Duration {
                    seconds: 1,
                    nanos: 0,
                }),
                output_size: 42,
                commands: vec![data::CommandExecution {
                    details: Some(data::CommandExecutionDetails {
                        signed_exit_code: Some(0),
                        ..Default::default()
                    }),
                    status: Some(data::command_execution::Status::Success(
                        data::command_execution::Success {},
                    )),
                    ..Default::default()
                }],
                outputs: Vec::new(),
                ..Default::default()
            },
        ))),
    }));

    let out = translate_buck_event(&ctx(), &event);
    assert_eq!(out.len(), 1);

    match out[0].payload.as_ref().expect("payload") {
        bep::build_event::Payload::Action(a) => {
            assert!(a.success);
            assert_eq!(a.exit_code, 0);
            assert_eq!(a.r#type, "cxx_compile");
        }
        _ => panic!("expected Action"),
    }
}

#[test]
fn test_run_end_becomes_test_result() {
    let event = make_event(data::buck_event::Data::SpanEnd(data::SpanEndEvent {
        stats: None,
        duration: Some(prost_types::Duration {
            seconds: 5,
            nanos: 0,
        }),
        data: Some(data::span_end_event::Data::TestEnd(data::TestRunEnd {
            suite: Some(data::TestSuite {
                suite_name: "my_test".to_owned(),
                test_names: vec!["case_a".to_owned()],
                target_label: Some(data::ConfiguredTargetLabel {
                    label: Some(data::TargetLabel {
                        package: "foo/bar".to_owned(),
                        name: "my_test".to_owned(),
                    }),
                    configuration: Some(data::Configuration {
                        full_name: "opt#abc123".to_owned(),
                    }),
                    execution_configuration: None,
                }),
            }),
            command_report: Some(data::CommandExecution {
                details: Some(data::CommandExecutionDetails {
                    signed_exit_code: Some(0),
                    ..Default::default()
                }),
                status: Some(data::command_execution::Status::Success(
                    data::command_execution::Success {},
                )),
                ..Default::default()
            }),
            command_host_sharing_requirements: None,
        })),
    }));

    let out = translate_buck_event(&ctx(), &event);
    assert_eq!(out.len(), 1);

    match &out[0].id.as_ref().expect("id").id {
        Some(bep::build_event_id::Id::TestResult(id)) => {
            assert_eq!(id.label, "//foo/bar:my_test");
            assert_eq!(
                id.configuration.as_ref().map(|c| c.id.as_str()),
                Some("opt#abc123")
            );
        }
        _ => panic!("expected TestResultId"),
    }

    match out[0].payload.as_ref().expect("payload") {
        bep::build_event::Payload::TestResult(r) => {
            assert_eq!(r.status, bep::TestStatus::Passed as i32);
            assert_eq!(r.test_attempt_duration.as_ref().map(|d| d.seconds), Some(5));
        }
        _ => panic!("expected TestResult"),
    }
}

#[test]
fn make_progress_carries_stdout_stderr() {
    let p = make_progress(3, "hello\n", "warn: x\n", Vec::new());
    match p.payload.as_ref().expect("payload") {
        bep::build_event::Payload::Progress(progress) => {
            assert_eq!(progress.stdout, "hello\n");
            assert_eq!(progress.stderr, "warn: x\n");
        }
        _ => panic!("expected Progress"),
    }
}
