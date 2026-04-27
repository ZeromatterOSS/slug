//! Translate Kuro's `BuckEvent` stream into Bazel's Build Event Protocol
//! (`build_event_stream.BuildEvent`).
//!
//! This layer is stateless per event: each translator function takes a single
//! Kuro event and returns zero or more BEP events. BEP consumers expect
//! parent/child relationships between events, but Kuro's dispatcher preserves
//! emission order and the *content* of each BEP event (its `id`, `children`,
//! and payload) can be constructed from a single Kuro event plus an
//! invocation-level `BuildEventContext`.
//!
//! Not every Kuro event maps cleanly to BEP. Dropped kinds are documented
//! inline.
//!
//! See `../../../../thoughts/shared/plans/kuro-bazel-subplans/18-bep-parity.md`
//! for the required coverage matrix.

// BEP includes deprecated fields (the `*_millis` timestamp pairs, legacy
// `label`/`configuration` on `ActionExecuted`, etc.) that the wire format
// still carries for pre-9.x consumers. Populate them at construction time;
// the lint fires once per field assignment.
#![allow(deprecated)]

use std::time::SystemTime;

use kuro_data as data;
use prost_types::Timestamp;

use crate::build_event_stream as bep;

/// Invocation-level constants threaded into BEP events that need them.
///
/// Created once at `CommandStart` from Kuro invocation metadata, then reused
/// for the rest of the stream.
#[derive(Debug, Clone, Default)]
pub struct BuildEventContext {
    /// Root cell name. Kuro's `TargetPattern.value` stores patterns with the
    /// declaring cell as a prefix (`hello_world//:main`); Bazel's BEP emits
    /// root-cell patterns without the prefix (`//:main`). When set, this
    /// field is stripped from pattern strings so BuildBuddy / `bep_diff`
    /// see the same shape Bazel emits.
    pub root_cell_name: String,
    /// UUIDv4 identifying the invocation. Kuro's `trace_id`. Surfaces as
    /// `BuildStarted.uuid` and forms part of the invocation URL on
    /// BuildBuddy-style dashboards.
    pub invocation_id: String,
    pub build_tool_version: String,
    pub workspace_directory: String,
    pub working_directory: String,
    pub user: String,
    pub host: String,
    pub command: String,
    pub cli_args: Vec<String>,
    pub server_pid: i64,
}

/// Entry point: translate one Kuro `BuckEvent` into zero or more BEP events.
///
/// Returns an empty vec for events with no BEP analogue.
pub fn translate_buck_event(
    ctx: &BuildEventContext,
    event: &data::BuckEvent,
) -> Vec<bep::BuildEvent> {
    let Some(ref data) = event.data else {
        return Vec::new();
    };
    let event_time = event.timestamp.clone();
    match data {
        data::buck_event::Data::SpanStart(start) => match start.data.as_ref() {
            Some(data::span_start_event::Data::Command(command)) => {
                translate_command_start(ctx, command, event_time.as_ref())
            }
            _ => Vec::new(),
        },
        data::buck_event::Data::SpanEnd(end) => match end.data.as_ref() {
            Some(data::span_end_event::Data::Command(command)) => {
                vec![translate_command_end(command, event_time.as_ref())]
            }
            Some(data::span_end_event::Data::ActionExecution(action)) => {
                vec![translate_action_execution_end(
                    action,
                    end.duration.as_ref(),
                    event_time.as_ref(),
                )]
            }
            Some(data::span_end_event::Data::Analysis(analysis)) => {
                translate_analysis_end(ctx, analysis)
            }
            Some(data::span_end_event::Data::TestEnd(test_end)) => {
                vec![translate_test_run_end(test_end, end.duration.as_ref())]
            }
            _ => Vec::new(),
        },
        data::buck_event::Data::Instant(instant) => match instant.data.as_ref() {
            Some(data::instant_event::Data::TargetPatterns(patterns)) => {
                vec![translate_parsed_target_patterns(ctx, patterns)]
            }
            Some(data::instant_event::Data::ConfigurationCreated(cfg)) => {
                vec![translate_configuration_created(cfg)]
            }
            _ => Vec::new(),
        },
        data::buck_event::Data::Record(_) => Vec::new(),
    }
}

/// Render a Kuro `TargetLabel` in Bazel's label syntax (`//pkg:name`).
///
/// Cells are not threaded through (Kuro `TargetLabel` lacks an explicit cell
/// field); for typical single-workspace builds this matches Bazel's default
/// `//pkg:name` rendering. Future work: surface cell as an `@cell` prefix when
/// it is not the root cell.
pub fn render_label(label: &data::TargetLabel) -> String {
    format!("//{}:{}", label.package, label.name)
}

/// Render a `ConfiguredTargetLabel` in Bazel style (drops configuration; BEP
/// carries configuration separately via `TargetConfiguredId.aspect`/
/// `TargetCompletedId.configuration`).
pub fn render_configured_label(label: &data::ConfiguredTargetLabel) -> String {
    label.label.as_ref().map(render_label).unwrap_or_default()
}

/// Extract the configuration's BEP-style id (= Kuro's `full_name`, an opaque
/// hash). Empty string when absent.
pub fn configuration_id(label: &data::ConfiguredTargetLabel) -> String {
    label
        .configuration
        .as_ref()
        .map(|c| c.full_name.clone())
        .unwrap_or_default()
}

/// `CommandStart` → the burst of metadata events Bazel emits at the start
/// of every invocation: `Started`, `BuildMetadata`, `UnstructuredCommandLine`,
/// three `StructuredCommandLine` views (original / canonical / tool),
/// `OptionsParsed`, and `WorkspaceStatus`. Together these populate the
/// pattern field, command bar, host/user chips, and Options tab on the
/// BuildBuddy invocation page; without them BB renders a near-empty card.
pub fn translate_command_start(
    ctx: &BuildEventContext,
    _command: &data::CommandStart,
    event_time: Option<&Timestamp>,
) -> Vec<bep::BuildEvent> {
    use bep::build_event_id as beid;

    let id = |inner: beid::Id| bep::BuildEventId { id: Some(inner) };

    // BuildBuddy extracts the invocation pattern from
    // `Started.children[i].id.pattern.pattern` — *not* from the standalone
    // `PatternExpanded` event's id. So we have to populate the patterns at
    // command-start time, before the kuro daemon has emitted its
    // `ParsedTargetPatterns` instant event. Best-effort: pull
    // target-shaped tokens out of the sanitized argv.
    let cli_patterns = extract_pattern_args(&ctx.cli_args);
    let mut started_children = Vec::with_capacity(9);
    if !cli_patterns.is_empty() {
        started_children.push(id(beid::Id::Pattern(beid::PatternExpandedId {
            pattern: cli_patterns,
        })));
    }
    started_children.extend([
        id(beid::Id::BuildMetadata(beid::BuildMetadataId {})),
        id(beid::Id::UnstructuredCommandLine(
            beid::UnstructuredCommandLineId {},
        )),
        id(beid::Id::StructuredCommandLine(
            beid::StructuredCommandLineId {
                command_line_label: "original".to_owned(),
            },
        )),
        id(beid::Id::StructuredCommandLine(
            beid::StructuredCommandLineId {
                command_line_label: "canonical".to_owned(),
            },
        )),
        id(beid::Id::StructuredCommandLine(
            beid::StructuredCommandLineId {
                command_line_label: "tool".to_owned(),
            },
        )),
        id(beid::Id::OptionsParsed(beid::OptionsParsedId {})),
        id(beid::Id::WorkspaceStatus(beid::WorkspaceStatusId {})),
        id(beid::Id::BuildFinished(beid::BuildFinishedId {})),
        id(beid::Id::BuildToolLogs(beid::BuildToolLogsId {})),
        id(beid::Id::BuildMetrics(beid::BuildMetricsId {})),
    ]);

    vec![
        bep::BuildEvent {
            id: Some(id(beid::Id::Started(beid::BuildStartedId {}))),
            children: started_children,
            last_message: false,
            payload: Some(bep::build_event::Payload::Started(bep::BuildStarted {
                uuid: ctx.invocation_id.clone(),
                start_time: event_time.cloned(),
                build_tool_version: ctx.build_tool_version.clone(),
                command: ctx.command.clone(),
                working_directory: ctx.working_directory.clone(),
                workspace_directory: ctx.workspace_directory.clone(),
                user: ctx.user.clone(),
                host: ctx.host.clone(),
                server_pid: ctx.server_pid,
                java_version_info: None,
                // BuildBuddy's `EventChannel.FinalizeInvocation`
                // early-returns when `!hasReceivedEventWithOptions`; that
                // flag flips on `Started.options_description != ""` or on
                // `OptionsParsed`. Populate it (we also emit a proper
                // `OptionsParsed` below).
                options_description: ctx.cli_args.join(" "),
                start_time_millis: timestamp_to_millis(event_time),
            })),
        },
        bep::BuildEvent {
            id: Some(id(beid::Id::BuildMetadata(beid::BuildMetadataId {}))),
            children: Vec::new(),
            last_message: false,
            payload: Some(bep::build_event::Payload::BuildMetadata(
                bep::BuildMetadata {
                    metadata: Default::default(),
                },
            )),
        },
        bep::BuildEvent {
            id: Some(id(beid::Id::UnstructuredCommandLine(
                beid::UnstructuredCommandLineId {},
            ))),
            children: Vec::new(),
            last_message: false,
            payload: Some(bep::build_event::Payload::UnstructuredCommandLine(
                bep::UnstructuredCommandLine {
                    args: ctx.cli_args.clone(),
                },
            )),
        },
        structured_command_line_event(ctx, "original"),
        structured_command_line_event(ctx, "canonical"),
        structured_command_line_event(ctx, "tool"),
        bep::BuildEvent {
            id: Some(id(beid::Id::OptionsParsed(beid::OptionsParsedId {}))),
            children: Vec::new(),
            last_message: false,
            payload: Some(bep::build_event::Payload::OptionsParsed(
                bep::OptionsParsed {
                    startup_options: Vec::new(),
                    explicit_startup_options: Vec::new(),
                    cmd_line: ctx.cli_args.clone(),
                    explicit_cmd_line: ctx.cli_args.clone(),
                    invocation_policy: None,
                    tool_tag: "kuro".to_owned(),
                },
            )),
        },
        bep::BuildEvent {
            id: Some(id(beid::Id::WorkspaceStatus(beid::WorkspaceStatusId {}))),
            children: Vec::new(),
            last_message: false,
            payload: Some(bep::build_event::Payload::WorkspaceStatus(
                bep::WorkspaceStatus {
                    item: workspace_status_items(ctx),
                },
            )),
        },
    ]
}

fn structured_command_line_event(ctx: &BuildEventContext, label: &str) -> bep::BuildEvent {
    use bep::build_event_id as beid;

    let id = beid::Id::StructuredCommandLine(beid::StructuredCommandLineId {
        command_line_label: label.to_owned(),
    });

    // Minimal viable shape: one `command` chunk with the subcommand name
    // followed by a `residual` chunk listing the user's args. Gives BB
    // enough structure to render the command bar without forcing us to
    // model startup options / canonical-vs-original yet.
    let sections = vec![
        crate::command_line::CommandLineSection {
            section_label: "command".to_owned(),
            section_type: Some(
                crate::command_line::command_line_section::SectionType::ChunkList(
                    crate::command_line::ChunkList {
                        chunk: vec![ctx.command.clone()],
                    },
                ),
            ),
        },
        crate::command_line::CommandLineSection {
            section_label: "residual".to_owned(),
            section_type: Some(
                crate::command_line::command_line_section::SectionType::ChunkList(
                    crate::command_line::ChunkList {
                        chunk: ctx.cli_args.clone(),
                    },
                ),
            ),
        },
    ];

    bep::BuildEvent {
        id: Some(bep::BuildEventId { id: Some(id) }),
        children: Vec::new(),
        last_message: false,
        payload: Some(bep::build_event::Payload::StructuredCommandLine(
            crate::command_line::CommandLine {
                command_line_label: label.to_owned(),
                sections,
            },
        )),
    }
}

fn workspace_status_items(ctx: &BuildEventContext) -> Vec<bep::workspace_status::Item> {
    let mk = |key: &str, value: &str| bep::workspace_status::Item {
        key: key.to_owned(),
        value: value.to_owned(),
    };
    let mut items = vec![
        mk("BUILD_HOST", &ctx.host),
        mk("BUILD_USER", &ctx.user),
        mk("BUILD_TOOL", "kuro"),
        mk("BUILD_TOOL_VERSION", &ctx.build_tool_version),
    ];
    items.retain(|i| !i.value.is_empty());
    items
}

/// `CommandEnd` → BEP `BuildFinished`. This is the last message in a
/// successful/failed (non-aborted) stream.
pub fn translate_command_end(
    command: &data::CommandEnd,
    event_time: Option<&Timestamp>,
) -> bep::BuildEvent {
    use bep::build_event_id as beid;

    let success = command
        .build_result
        .as_ref()
        .map(|r| r.build_completed)
        .unwrap_or(command.is_success);
    let exit_code = if success { 0 } else { 1 };
    bep::BuildEvent {
        id: Some(bep::BuildEventId {
            id: Some(beid::Id::BuildFinished(beid::BuildFinishedId {})),
        }),
        children: Vec::new(),
        // `BuildFinished` is followed by `BuildMetrics` in the Bazel
        // stream, so it is no longer the terminal event.
        last_message: false,
        payload: Some(bep::build_event::Payload::Finished(bep::BuildFinished {
            overall_success: success,
            exit_code: Some(bep::build_finished::ExitCode {
                name: if success { "SUCCESS" } else { "BUILD_FAILURE" }.to_owned(),
                code: exit_code,
            }),
            finish_time: event_time.cloned(),
            finish_time_millis: timestamp_to_millis(event_time),
            anomaly_report: None,
            failure_detail: None,
        })),
    }
}

/// `ActionExecutionEnd` → BEP `ActionExecuted`.
///
/// Bazel conventionally omits successful actions from the BEP stream, but the
/// spec allows them; BuildBuddy displays all actions when they are posted.
///
/// `start_time` and `end_time` populate BuildBuddy's action timing /
/// critical-path graph. End is the event's emission timestamp; start is
/// derived by subtracting `wall_time` (which kuro records on the action).
pub fn translate_action_execution_end(
    action: &data::ActionExecutionEnd,
    duration: Option<&prost_types::Duration>,
    event_time: Option<&Timestamp>,
) -> bep::BuildEvent {
    use bep::build_event_id as beid;

    let action_id = action
        .key
        .as_ref()
        .map(action_key_id)
        .unwrap_or_else(|| "<unknown>".to_owned());

    let exit_code = action
        .commands
        .last()
        .and_then(|c| c.details.as_ref())
        .and_then(|d| d.signed_exit_code)
        .unwrap_or(if action.failed { 1 } else { 0 });

    let end_time = event_time.cloned();
    // Prefer the action's own wall_time; fall back to span duration.
    let span_duration = action.wall_time.as_ref().or(duration);
    let start_time = end_time
        .as_ref()
        .zip(span_duration)
        .map(|(end, dur)| timestamp_minus_duration(end, dur));

    bep::BuildEvent {
        id: Some(bep::BuildEventId {
            id: Some(beid::Id::ActionCompleted(beid::ActionCompletedId {
                primary_output: String::new(),
                label: String::new(),
                configuration: None,
            })),
        }),
        children: Vec::new(),
        last_message: false,
        payload: Some(bep::build_event::Payload::Action(bep::ActionExecuted {
            success: !action.failed,
            r#type: action
                .name
                .as_ref()
                .map(|n| n.category.clone())
                .unwrap_or_else(|| action_id.clone()),
            exit_code,
            stdout: None,
            stderr: None,
            label: String::new(),
            configuration: None,
            primary_output: None,
            command_line: Vec::new(),
            failure_detail: None,
            start_time,
            end_time,
            strategy_details: Vec::new(),
        })),
    }
}

/// Stream-level state accumulated as `BuckEvent`s flow through the
/// translator. Subscribers compose this to emit synthetic BEP events that
/// require cross-event aggregation (currently `BuildMetrics`).
#[derive(Debug, Default)]
pub struct BepStreamState {
    actions_executed: i64,
    targets_configured: i64,
    first_action_started_ms: i64,
    last_action_ended_ms: i64,
    /// Per-action records (name, start_us, dur_us, category) used to
    /// synthesize a Chrome-trace `command.profile.gz` for BuildBuddy's
    /// Timing tab. The Timing tab reads from a `BuildToolLogs.File`
    /// named `command.profile.gz` (Bazel convention) — without it the
    /// tab renders blank even when `BuildMetrics.action_data` carries
    /// the same numbers.
    action_traces: Vec<TraceEvent>,
}

#[derive(Clone, Debug)]
struct TraceEvent {
    name: String,
    category: String,
    /// Start in microseconds since epoch.
    ts_us: i64,
    /// Duration in microseconds.
    dur_us: i64,
}

impl BepStreamState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Walk a translated BEP event and update internal counters. Call this
    /// after `translate_buck_event` for each emitted BEP event so the
    /// final `BuildMetrics` reflects what we actually streamed.
    pub fn observe(&mut self, event: &bep::BuildEvent) {
        match event.payload.as_ref() {
            Some(bep::build_event::Payload::Action(action)) => {
                self.actions_executed += 1;
                let start_us = action
                    .start_time
                    .as_ref()
                    .map(|t| t.seconds * 1_000_000 + i64::from(t.nanos / 1_000));
                let end_us = action
                    .end_time
                    .as_ref()
                    .map(|t| t.seconds * 1_000_000 + i64::from(t.nanos / 1_000));
                if let Some(start) = start_us {
                    let ms = start / 1_000;
                    if self.first_action_started_ms == 0 || ms < self.first_action_started_ms {
                        self.first_action_started_ms = ms;
                    }
                }
                if let Some(end) = end_us {
                    let ms = end / 1_000;
                    if ms > self.last_action_ended_ms {
                        self.last_action_ended_ms = ms;
                    }
                }
                if let (Some(start), Some(end)) = (start_us, end_us) {
                    let dur = (end - start).max(0);
                    let label = action.label.clone();
                    let mnemonic = if action.r#type.is_empty() {
                        "Action".to_owned()
                    } else {
                        action.r#type.clone()
                    };
                    let name = if label.is_empty() {
                        mnemonic.clone()
                    } else {
                        format!("{} {}", mnemonic, label)
                    };
                    self.action_traces.push(TraceEvent {
                        name,
                        category: mnemonic,
                        ts_us: start,
                        dur_us: dur,
                    });
                }
            }
            Some(bep::build_event::Payload::Configured(_)) => {
                self.targets_configured += 1;
            }
            _ => {}
        }
    }

    /// Synthesize the `BuildToolLogs` event Bazel emits between
    /// `BuildFinished` and the closing `BuildMetrics`. The single
    /// `command.profile.gz` log file we emit is a chrome-trace JSON
    /// (gzipped) built from the per-action timestamps `observe`
    /// recorded — BuildBuddy's Timing tab reads this file and renders
    /// the action timeline / critical path. Without this event the
    /// tab renders blank even when `BuildMetrics.action_data` carries
    /// the same numbers (BB's frontend hardcodes the trace JSON path).
    pub fn build_tool_logs_event(&self) -> Option<bep::BuildEvent> {
        use bep::build_event_id as beid;

        if self.action_traces.is_empty() {
            return None;
        }

        // Match Bazel's `JsonTraceFileWriter` output shape exactly so
        // BuildBuddy's Timing tab parser doesn't reject it as a
        // non-default profile:
        //
        //   {
        //     "otherData": { … },
        //     "traceEvents": [
        //       {"ph":"M","pid":1,"tid":0,"name":"process_name","args":{"name":"action_count"}},
        //       {"ph":"M","pid":1,"tid":0,"name":"thread_name","args":{"name":"Critical Path"}},
        //       {"ph":"M","pid":1,"tid":0,"name":"thread_sort_index","args":{"sort_index":0}},
        //       {"ph":"X", "cat":…, "name":…, "ts":…, "dur":…, "pid":1,"tid":0, "args":{}},
        //       …
        //     ]
        //   }
        //
        // BB looks for the bare-array form too but rejects empty trace
        // streams with "Could not find profile info." A wrapped object
        // with metadata events sidesteps that path entirely.
        let mut json = String::with_capacity(256 + self.action_traces.len() * 128);
        json.push_str(r#"{"otherData":{"build_tool":"kuro"},"traceEvents":["#);
        // Metadata events naming the lane.
        json.push_str(
            r#"{"ph":"M","pid":1,"tid":0,"name":"process_name","args":{"name":"actions"}}"#,
        );
        json.push_str(
            r#",{"ph":"M","pid":1,"tid":0,"name":"thread_name","args":{"name":"Actions"}}"#,
        );
        json.push_str(
            r#",{"ph":"M","pid":1,"tid":0,"name":"thread_sort_index","args":{"sort_index":0}}"#,
        );
        // Per-action duration events. Timestamps shift to a build-relative
        // origin so the trace starts near zero; absolute epoch
        // microseconds parse fine but make Bazel's timeline UI start at
        // year-1970-relative offsets that look wrong in BB.
        let origin_us = self
            .action_traces
            .iter()
            .map(|e| e.ts_us)
            .min()
            .unwrap_or(0);
        for e in &self.action_traces {
            json.push(',');
            let ts = e.ts_us.saturating_sub(origin_us);
            json.push_str(&format!(
                "{{\"ph\":\"X\",\"cat\":{cat},\"name\":{name},\"ts\":{ts},\"dur\":{dur},\"pid\":1,\"tid\":0,\"args\":{{}}}}",
                name = json_string(&e.name),
                cat = json_string(&e.category),
                ts = ts,
                dur = e.dur_us,
            ));
        }
        json.push_str("]}");

        // Gzip the JSON. `command.profile.gz` is the Bazel filename BB
        // recognizes; payload must be gzip-compressed.
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        use std::io::Write;
        if gz.write_all(json.as_bytes()).is_err() {
            return None;
        }
        let contents = match gz.finish() {
            Ok(buf) => buf,
            Err(_) => return None,
        };

        Some(bep::BuildEvent {
            id: Some(bep::BuildEventId {
                id: Some(beid::Id::BuildToolLogs(beid::BuildToolLogsId {})),
            }),
            children: Vec::new(),
            last_message: false,
            payload: Some(bep::build_event::Payload::BuildToolLogs(
                bep::BuildToolLogs {
                    log: vec![bep::File {
                        path_prefix: Vec::new(),
                        name: "command.profile.gz".to_owned(),
                        digest: String::new(),
                        length: contents.len() as i64,
                        file: Some(bep::file::File::Contents(contents)),
                    }],
                },
            )),
        })
    }

    /// Synthesize the closing `BuildMetrics` event Bazel emits last in its
    /// BEP stream. BuildBuddy reads `actions_executed` for its action-count
    /// chip + critical-path graph, and `targets_configured` for the
    /// per-target summary header.
    pub fn build_metrics_event(&self) -> bep::BuildEvent {
        use bep::build_event_id as beid;

        let mut action_data = Vec::new();
        if self.actions_executed > 0 {
            action_data.push(bep::build_metrics::action_summary::ActionData {
                mnemonic: String::new(),
                actions_executed: self.actions_executed,
                first_started_ms: self.first_action_started_ms,
                last_ended_ms: self.last_action_ended_ms,
                system_time: None,
                user_time: None,
                actions_created: self.actions_executed,
            });
        }

        bep::BuildEvent {
            id: Some(bep::BuildEventId {
                id: Some(beid::Id::BuildMetrics(beid::BuildMetricsId {})),
            }),
            children: Vec::new(),
            // Terminal event in the BEP stream; matches Bazel's ordering
            // (`BuildFinished` → `BuildToolLogs` → `BuildMetrics last=true`).
            last_message: true,
            payload: Some(bep::build_event::Payload::BuildMetrics(bep::BuildMetrics {
                action_summary: Some(bep::build_metrics::ActionSummary {
                    actions_created: self.actions_executed,
                    actions_created_not_including_aspects: self.actions_executed,
                    actions_executed: self.actions_executed,
                    action_data,
                    remote_cache_hits: 0,
                    runner_count: Vec::new(),
                    action_cache_statistics: None,
                }),
                memory_metrics: None,
                target_metrics: Some(bep::build_metrics::TargetMetrics {
                    targets_loaded: self.targets_configured,
                    targets_configured: self.targets_configured,
                    targets_configured_not_including_aspects: self.targets_configured,
                }),
                package_metrics: None,
                timing_metrics: None,
                cumulative_metrics: None,
                artifact_metrics: None,
                build_graph_metrics: None,
                worker_metrics: Vec::new(),
                network_metrics: None,
                worker_pool_metrics: None,
                dynamic_execution_metrics: None,
                remote_analysis_cache_statistics: None,
            })),
        }
    }
}

/// Minimal JSON-string escaper for the chrome-trace name/cat fields.
/// We don't pull `serde_json` in just for this — the inputs come from
/// `data::ActionExecutionEnd.label` / `.type`, which can contain
/// quotes, backslashes, and (rarely) control characters in label
/// strings. Output is wrapped in double quotes.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Pull out tokens from a sanitized argv that look like Bazel target
/// patterns. Used at command-start to populate the `Started.children[]
/// .pattern` shape BuildBuddy reads its top-level invocation pattern from.
///
/// We err on the inclusive side: anything starting with `//`, `@`, or `:`
/// counts. Excludes flags (`--*`/`-*`), the executable name, the
/// subcommand, and any token matching the workspace-relative `--key=value`
/// shape.
fn extract_pattern_args(argv: &[String]) -> Vec<String> {
    argv.iter()
        .filter(|a| {
            let a = a.as_str();
            (a.starts_with("//") || a.starts_with("@") || a.starts_with(':')) && !a.contains('=')
        })
        .cloned()
        .collect()
}

/// Strip the root cell name prefix from a target pattern. Kuro stores
/// patterns as `<cell>//pkg:name`; Bazel's BEP uses `//pkg:name` for the
/// root cell and `@cell//pkg:name` for external cells. Returning the
/// Bazel form keeps BuildBuddy's pattern-extraction code happy and makes
/// `bep_diff` output line up.
fn normalize_root_cell_pattern(ctx: &BuildEventContext, pattern: &str) -> String {
    if !ctx.root_cell_name.is_empty()
        && let Some(rest) = pattern.strip_prefix(&ctx.root_cell_name)
        && rest.starts_with("//")
    {
        return rest.to_owned();
    }
    pattern.to_owned()
}

/// Subtract a `Duration` from a `Timestamp`, producing the start time of an
/// event whose end time and elapsed duration are known. Used for action
/// timing graphs where kuro records `wall_time` only.
fn timestamp_minus_duration(end: &Timestamp, dur: &prost_types::Duration) -> Timestamp {
    let mut sec = end.seconds - dur.seconds;
    let mut nanos = end.nanos - dur.nanos;
    if nanos < 0 {
        sec -= 1;
        nanos += 1_000_000_000;
    }
    if nanos >= 1_000_000_000 {
        sec += 1;
        nanos -= 1_000_000_000;
    }
    Timestamp {
        seconds: sec,
        nanos,
    }
}

fn action_key_id(_key: &data::ActionKey) -> String {
    // BEP doesn't give us a good stable-id slot for an action (its id is the
    // label + primary output + configuration). Using a placeholder until the
    // callsite supplies the owning target label — see 18.3 for the caller.
    "<action>".to_owned()
}

/// `AnalysisEnd` → BEP `TargetConfigured` + `TargetCompleted`.
///
/// Bazel emits `Completed` events at materialization time; we don't yet
/// thread the per-target output set through the translator, so we synthesize
/// a `TargetCompleted{success=true}` alongside `TargetConfigured` so that
/// BuildBuddy's per-target summary cards have something to render. Output
/// groups stay empty until per-target artifact tracking lands (Plan 18.2
/// gap list).
pub fn translate_analysis_end(
    ctx: &BuildEventContext,
    analysis: &data::AnalysisEnd,
) -> Vec<bep::BuildEvent> {
    use bep::build_event_id as beid;

    let (label, aspect, configuration_id_str) = match analysis.target.as_ref() {
        Some(data::analysis_end::Target::StandardTarget(t)) => (
            normalize_root_cell_pattern(ctx, &render_configured_label(t)),
            String::new(),
            configuration_id(t),
        ),
        Some(data::analysis_end::Target::AnonTarget(_))
        | Some(data::analysis_end::Target::DynamicLambda(_))
        | None => (String::new(), String::new(), String::new()),
    };

    let target_completed_id = beid::TargetCompletedId {
        label: label.clone(),
        configuration: Some(beid::ConfigurationId {
            id: configuration_id_str.clone(),
        }),
        aspect: aspect.clone(),
    };

    vec![
        bep::BuildEvent {
            id: Some(bep::BuildEventId {
                id: Some(beid::Id::TargetConfigured(beid::TargetConfiguredId {
                    label: label.clone(),
                    aspect: aspect.clone(),
                })),
            }),
            children: vec![bep::BuildEventId {
                id: Some(beid::Id::TargetCompleted(target_completed_id.clone())),
            }],
            last_message: false,
            payload: Some(bep::build_event::Payload::Configured(
                bep::TargetConfigured {
                    target_kind: analysis.rule.clone(),
                    test_size: bep::TestSize::Unknown as i32,
                    tag: Vec::new(),
                },
            )),
        },
        bep::BuildEvent {
            id: Some(bep::BuildEventId {
                id: Some(beid::Id::TargetCompleted(target_completed_id)),
            }),
            children: Vec::new(),
            last_message: false,
            payload: Some(bep::build_event::Payload::Completed(bep::TargetComplete {
                success: true,
                target_kind: analysis.rule.clone(),
                test_size: bep::TestSize::Unknown as i32,
                output_group: Vec::new(),
                important_output: Vec::new(),
                directory_output: Vec::new(),
                tag: Vec::new(),
                test_timeout_seconds: 0,
                test_timeout: None,
                failure_detail: None,
            })),
        },
    ]
}

/// `TestRunEnd` → BEP `TestResult`.
pub fn translate_test_run_end(
    test_end: &data::TestRunEnd,
    duration: Option<&prost_types::Duration>,
) -> bep::BuildEvent {
    use bep::build_event_id as beid;

    let (label, configuration_id) = test_end
        .suite
        .as_ref()
        .and_then(|s| s.target_label.as_ref())
        .map(|t| (render_configured_label(t), configuration_id(t)))
        .unwrap_or_default();

    let status = test_status_from_command_execution(test_end.command_report.as_ref());

    bep::BuildEvent {
        id: Some(bep::BuildEventId {
            id: Some(beid::Id::TestResult(beid::TestResultId {
                label,
                run: 1,
                shard: 1,
                attempt: 1,
                configuration: Some(beid::ConfigurationId {
                    id: configuration_id,
                }),
            })),
        }),
        children: Vec::new(),
        last_message: false,
        payload: Some(bep::build_event::Payload::TestResult(bep::TestResult {
            status: status as i32,
            status_details: String::new(),
            cached_locally: false,
            test_attempt_start: None,
            test_attempt_duration: duration.cloned(),
            test_action_output: Vec::new(),
            warning: Vec::new(),
            execution_info: None,
            test_attempt_start_millis_epoch: 0,
            test_attempt_duration_millis: duration
                .map(|d| d.seconds * 1_000 + i64::from(d.nanos / 1_000_000))
                .unwrap_or_default(),
        })),
    }
}

fn test_status_from_command_execution(cmd: Option<&data::CommandExecution>) -> bep::TestStatus {
    match cmd.and_then(|c| c.status.as_ref()) {
        Some(data::command_execution::Status::Success(_)) => bep::TestStatus::Passed,
        Some(data::command_execution::Status::Failure(_)) => bep::TestStatus::Failed,
        Some(data::command_execution::Status::Timeout(_)) => bep::TestStatus::Timeout,
        _ => bep::TestStatus::NoStatus,
    }
}

/// `ParsedTargetPatterns` (instant) → BEP `PatternExpanded`.
pub fn translate_parsed_target_patterns(
    ctx: &BuildEventContext,
    patterns: &data::ParsedTargetPatterns,
) -> bep::BuildEvent {
    use bep::build_event_id as beid;

    let pattern_ids: Vec<String> = patterns
        .target_patterns
        .iter()
        .map(|p| normalize_root_cell_pattern(ctx, &p.value))
        .collect();

    bep::BuildEvent {
        id: Some(bep::BuildEventId {
            id: Some(beid::Id::Pattern(beid::PatternExpandedId {
                pattern: pattern_ids,
            })),
        }),
        children: Vec::new(),
        last_message: false,
        payload: Some(bep::build_event::Payload::Expanded(bep::PatternExpanded {
            test_suite_expansions: Vec::new(),
        })),
    }
}

/// `ConfigurationCreated` (instant) → BEP `Configuration`.
pub fn translate_configuration_created(cfg: &data::ConfigurationCreated) -> bep::BuildEvent {
    use bep::build_event_id as beid;

    let full_name = cfg
        .cfg
        .as_ref()
        .map(|c| c.full_name.clone())
        .unwrap_or_default();

    bep::BuildEvent {
        id: Some(bep::BuildEventId {
            id: Some(beid::Id::Configuration(beid::ConfigurationId {
                id: full_name.clone(),
            })),
        }),
        children: Vec::new(),
        last_message: false,
        payload: Some(bep::build_event::Payload::Configuration(
            bep::Configuration {
                mnemonic: full_name,
                platform_name: String::new(),
                cpu: String::new(),
                make_variable: Default::default(),
                is_tool: false,
            },
        )),
    }
}

/// Build a synthetic `Aborted` event, used on cancel/timeout. The caller
/// chooses an id that identifies which announced child event was skipped.
///
/// For top-level build aborts, use `BuildFinishedId` so that consumers see the
/// stream as closed rather than waiting for a never-delivered `BuildFinished`.
pub fn make_aborted(
    id: bep::BuildEventId,
    reason: bep::aborted::AbortReason,
    description: impl Into<String>,
) -> bep::BuildEvent {
    bep::BuildEvent {
        id: Some(id),
        children: Vec::new(),
        last_message: true,
        payload: Some(bep::build_event::Payload::Aborted(bep::Aborted {
            reason: reason as i32,
            description: description.into(),
        })),
    }
}

/// Build a `Progress` event carrying accumulated stdout/stderr since the last
/// tick. Progress events are the hook for "in-flight" action counts and log
/// chunks in BuildBuddy's UI.
///
/// `opaque_count` threads a monotonically increasing integer through the
/// progress-event id so each tick has a distinct `BuildEventId`.
pub fn make_progress(
    opaque_count: i32,
    stdout: impl Into<String>,
    stderr: impl Into<String>,
    children: Vec<bep::BuildEventId>,
) -> bep::BuildEvent {
    use bep::build_event_id as beid;

    bep::BuildEvent {
        id: Some(bep::BuildEventId {
            id: Some(beid::Id::Progress(beid::ProgressId { opaque_count })),
        }),
        children,
        last_message: false,
        payload: Some(bep::build_event::Payload::Progress(bep::Progress {
            stdout: stdout.into(),
            stderr: stderr.into(),
        })),
    }
}

/// Convert an optional `SystemTime` to a `Timestamp`. Useful for synthesizing
/// events that don't have a natural Kuro counterpart (e.g., `Aborted`).
pub fn system_time_to_timestamp(time: SystemTime) -> Timestamp {
    let d = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    Timestamp {
        seconds: d.as_secs() as i64,
        nanos: d.subsec_nanos() as i32,
    }
}

fn timestamp_to_millis(t: Option<&Timestamp>) -> i64 {
    t.map(|t| t.seconds * 1_000 + i64::from(t.nanos / 1_000_000))
        .unwrap_or_default()
}
