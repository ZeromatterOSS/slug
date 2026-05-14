//! Translate Slug's `BuckEvent` stream into Bazel's Build Event Protocol
//! (`build_event_stream.BuildEvent`).
//!
//! This layer is stateless per event: each translator function takes a single
//! Slug event and returns zero or more BEP events. BEP consumers expect
//! parent/child relationships between events, but Slug's dispatcher preserves
//! emission order and the *content* of each BEP event (its `id`, `children`,
//! and payload) can be constructed from a single Slug event plus an
//! invocation-level `BuildEventContext`.
//!
//! Not every Slug event maps cleanly to BEP. Dropped kinds are documented
//! inline.
//!
//! See `../../../../thoughts/shared/plans/slug-bazel-subplans/18-bep-parity.md`
//! for the required coverage matrix.

// BEP includes deprecated fields (the `*_millis` timestamp pairs, legacy
// `label`/`configuration` on `ActionExecuted`, etc.) that the wire format
// still carries for pre-9.x consumers. Populate them at construction time;
// the lint fires once per field assignment.
#![allow(deprecated)]

use std::time::SystemTime;

use prost_types::Timestamp;
use slug_data as data;

use crate::build_event_stream as bep;

/// Invocation-level constants threaded into BEP events that need them.
///
/// Created once at `CommandStart` from Slug invocation metadata, then reused
/// for the rest of the stream.
#[derive(Debug, Clone, Default)]
pub struct BuildEventContext {
    /// Root cell name. Slug's `TargetPattern.value` stores patterns with the
    /// declaring cell as a prefix (`hello_world//:main`); Bazel's BEP emits
    /// root-cell patterns without the prefix (`//:main`). When set, this
    /// field is stripped from pattern strings so BuildBuddy / `bep_diff`
    /// see the same shape Bazel emits.
    pub root_cell_name: String,
    /// UUIDv4 identifying the invocation. Slug's `trace_id`. Surfaces as
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

/// Entry point: translate one Slug `BuckEvent` into zero or more BEP events.
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

/// Render a Slug `TargetLabel` in Bazel's label syntax (`//pkg:name`).
///
/// Cells are not threaded through (Slug `TargetLabel` lacks an explicit cell
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

/// Extract the configuration's BEP-style id (= Slug's `full_name`, an opaque
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
    // command-start time, before the slug daemon has emitted its
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
                    tool_tag: "slug".to_owned(),
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
        mk("BUILD_TOOL", "slug"),
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
/// derived by subtracting `wall_time` (which slug records on the action).
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

    let owner_label = action
        .key
        .as_ref()
        .map(action_owner_label)
        .unwrap_or_default();

    bep::BuildEvent {
        id: Some(bep::BuildEventId {
            id: Some(beid::Id::ActionCompleted(beid::ActionCompletedId {
                primary_output: String::new(),
                label: owner_label.clone(),
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
            label: owner_label,
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
    /// Invocation UUID used to populate the chrome trace's
    /// `otherData.build_id`. BB cross-references this with the
    /// invocation_id in the BES stream — for large traces (clang's
    /// 4326 actions) BB's Timing tab gets stuck in "Build is in
    /// progress…" if the build_id doesn't match. Defaults to "slug"
    /// for tests / contexts without a real invocation_id.
    build_id: String,
    /// Build start time as ISO 8601 (e.g.
    /// `"2026-04-27T23:43:25.675932295Z"`) for the chrome trace's
    /// `otherData.date`. Captured at construction. Bazel sets the same
    /// field; BB's Timing parser uses it to anchor the timeline.
    profile_start_iso: String,
    /// Build start time as milliseconds since the Unix epoch, mirroring
    /// Bazel's `otherData.profile_start_ts`. Same purpose as
    /// `profile_start_iso` but in numeric form.
    profile_start_ms: i64,
    /// Local action-cache hit count
    /// (`ActionExecutionKind::ActionCache`). Drives the
    /// "Local Action Cache Hits" counter on BB's invocation page,
    /// surfaced via `BuildMetrics.action_summary.action_cache_statistics`.
    local_action_cache_hits: i32,
    /// Actions that did NOT come from the local action cache
    /// (i.e. ran locally / remotely or hit a remote cache). Recorded
    /// as cache `misses` per the `ActionCacheStatistics` proto.
    local_action_cache_misses: i32,
    /// Per-mnemonic action stats. Bazel emits one
    /// `ActionSummary.ActionData` row per mnemonic; BB uses this to
    /// populate the per-mnemonic action breakdown. Keyed by mnemonic
    /// string (e.g. `c_compile`, `cpp_link`).
    actions_by_mnemonic: std::collections::HashMap<String, MnemonicStats>,
    /// Per-executor-kind action counts ("local", "remote",
    /// "remote-cache", etc.). Maps to Bazel's
    /// `ActionSummary.runner_count[]` rows on BB's invocation page —
    /// the breakdown chip that shows e.g. "1 local, 3499 remote".
    runner_counts: std::collections::HashMap<String, i32>,
    /// Sum of action wall-time durations in microseconds. Used as a
    /// rough stand-in for Bazel's `TimingMetrics.cpu_time_in_ms` —
    /// slug doesn't separately track per-action OS-level CPU time, so
    /// the sum of action wall durations is the best we can do without
    /// additional per-action getrusage() instrumentation.
    total_action_wall_us: i64,
    /// Critical-path duration in microseconds, sourced from slug's
    /// terminal `BuildGraphExecutionInfo` instant event. Drives
    /// Bazel's `TimingMetrics.critical_path_time` field, which BB's
    /// invocation page renders as the "Critical path: 2m43s" chip.
    critical_path_us: u64,
    /// Action-graph node + edge counts from
    /// `BuildGraphExecutionInfo`. Surface as
    /// `BuildMetrics.build_graph_metrics.action_lookup_value_count` /
    /// `action_count` so BB's invocation page can show the build
    /// graph size. Approximations — BB's nomenclature treats these
    /// as Bazel SkyFunction nodes, slug's are DICE nodes; we map
    /// them 1:1 since the user-visible counts roughly correspond.
    graph_num_nodes: u64,
    graph_num_edges: u64,
    /// Wall-clock span of the top-level `Command` span — captured at
    /// `CommandStart` and finalised at `CommandEnd`. Used as
    /// `TimingMetrics.wall_time_in_ms` (overrides the action-span
    /// estimate so analysis-only / cached-only invocations still get
    /// a meaningful wall time).
    command_start_us: i64,
    command_end_us: i64,
    /// Periodic system snapshots (rss, cpu, load, network) the slug
    /// daemon emits as `Instant::Snapshot` events. Drives the chrome
    /// trace's counter (`ph: "C"`) events that BB renders as the
    /// time-series line plots panel between the flamegraph and the
    /// Timing Breakdown card. Without this panel the trace viewer
    /// silently drops it (line plots panel is filtered out when
    /// empty), which is why slug builds previously had only the
    /// flamegraph + breakdown but no metrics-over-time plots.
    snapshots: Vec<SnapshotSample>,
}

#[derive(Debug, Clone, Default)]
struct SnapshotSample {
    /// Microseconds since epoch.
    ts_us: i64,
    /// Daemon RSS in bytes. Drives "Memory usage (Bazel)" series (MB).
    slug_rss_bytes: u64,
    /// Daemon user+system CPU time accumulated. Differenced across
    /// consecutive samples to derive "CPU usage (Bazel)" (cores).
    slug_cpu_us: u64,
    /// Host user+system CPU time accumulated since command start
    /// (multi-core, may exceed wall by N cores). Differenced for
    /// "CPU usage (total)" (cores).
    host_cpu_ms: u64,
    /// 1-minute system load average. Direct value to "System load
    /// average" series.
    load1: f64,
    /// Sum of `tx_bytes` across all interfaces. Differenced for
    /// "Network Up usage (total)" (Mbps).
    net_tx_bytes: u64,
    /// Sum of `rx_bytes` across all interfaces. Differenced for
    /// "Network Down usage (total)" (Mbps).
    net_rx_bytes: u64,
}

#[derive(Debug, Default, Clone)]
struct MnemonicStats {
    /// Total executed actions of this mnemonic. Cache hits count
    /// (matching Bazel's `ActionData.actions_executed` semantics:
    /// "includes remote cache hits but excludes local action cache
    /// hits"). Filtered at observation time so this only counts
    /// non-local-cache actions.
    actions_executed: i64,
    /// Earliest start_time_ms across this mnemonic's action events.
    /// Stays at `i64::MAX` until the first event arrives so a `min()`
    /// pattern works correctly.
    first_started_ms: i64,
    /// Latest end_time_ms across this mnemonic's action events.
    last_ended_ms: i64,
}

#[derive(Clone, Debug)]
struct TraceEvent {
    /// Bazel-shape descriptive event name (e.g.
    /// `Compiling external/llvm-project/llvm/lib/Foo.cpp`).
    /// For our actions we currently produce `<mnemonic> <label>` since
    /// slug doesn't track the human-readable action description that
    /// rules_cc would synthesize. BB's Timing tab uses this for the
    /// per-event tooltip.
    name: String,
    /// Chrome trace `cat` (event category). Action events use the
    /// literal string `"action processing"` — Bazel's convention; BB
    /// filters by it to identify action events in the Timing tab.
    category: String,
    /// The action's mnemonic (e.g. `c_compile`, `cpp_link`,
    /// `Genrule`). Embedded as `args.mnemonic` in the chrome trace
    /// event JSON, matching Bazel's shape. Non-action events (test,
    /// general) leave this empty and the JSON omits `args`.
    mnemonic: String,
    /// Start in microseconds since epoch.
    ts_us: i64,
    /// Duration in microseconds.
    dur_us: i64,
    /// Chrome trace `tid`. Sourced from the slug action's
    /// `local_thread_id` (the OS thread id of the worker that polled
    /// the action's completion). `0` means "not captured" — the
    /// trace renderer skips emitting events with tid 0 for actions
    /// (tid 0 is reserved for the Critical Path lane in our schema).
    tid: u64,
    /// OS thread name (e.g. `slug-rt-3`). Used as the chrome trace
    /// `thread_name` metadata for this `tid` so BB's Timing tab shows
    /// "slug-rt-3" instead of a bare "Worker 3" placeholder. Empty
    /// when the thread had no name (test fixtures, callers without a
    /// tokio runtime).
    thread_name: String,
    /// Slug `ActionExecutionKind` raw int — drives the companion
    /// "breakdown-friendly" trace event name BB's Timing tab's
    /// Execution Breakdown card filters on
    /// (`subprocess.run` / `execute remotely` / `check cache hit`).
    /// 0 means "not an action event"; no companion gets emitted.
    execution_kind: i32,
}

impl BepStreamState {
    pub fn new() -> Self {
        let (iso, ms) = current_time_pair();
        Self {
            build_id: "slug".to_owned(),
            profile_start_iso: iso,
            profile_start_ms: ms,
            ..Self::default()
        }
    }

    /// Construct a state whose chrome trace's `otherData.build_id`
    /// matches the BES invocation_id. BB's Timing tab cross-references
    /// these — without the match, large invocations stay stuck in
    /// "Build is in progress…" even after the BuildFinished /
    /// BuildMetrics events arrive.
    pub fn with_invocation_id(invocation_id: String) -> Self {
        let (iso, ms) = current_time_pair();
        Self {
            build_id: invocation_id,
            profile_start_iso: iso,
            profile_start_ms: ms,
            ..Self::default()
        }
    }

    /// Walk a translated BEP event and update internal counters. Call this
    /// after `translate_buck_event` for each emitted BEP event so the
    /// final `BuildMetrics` reflects what we actually streamed.
    ///
    /// Observe a raw slug `BuckEvent` (regardless of whether it
    /// translates to a BEP event). Picks up data that's only on the
    /// slug side — `BuildGraphExecutionInfo` for critical-path stats
    /// and graph sizing, plus top-level `Command` span timestamps for
    /// the invocation wall time. Idempotent and safe to call before
    /// `observe`.
    pub fn observe_slug_event(&mut self, event: &data::BuckEvent) {
        let ts_us = event
            .timestamp
            .as_ref()
            .map(|t| t.seconds * 1_000_000 + i64::from(t.nanos / 1_000));
        match event.data.as_ref() {
            Some(data::buck_event::Data::SpanStart(span)) => {
                if let Some(data::span_start_event::Data::Command(_)) = span.data.as_ref() {
                    if let Some(t) = ts_us {
                        // Latest CommandStart wins — there should be
                        // at most one in a normal invocation.
                        self.command_start_us = t;
                    }
                }
            }
            Some(data::buck_event::Data::SpanEnd(span)) => {
                if let Some(data::span_end_event::Data::Command(_)) = span.data.as_ref() {
                    if let Some(t) = ts_us {
                        self.command_end_us = t;
                    }
                }
            }
            Some(data::buck_event::Data::Instant(instant)) => match instant.data.as_ref() {
                Some(data::instant_event::Data::BuildGraphInfo(info)) => {
                    self.graph_num_nodes = info.num_nodes;
                    self.graph_num_edges = info.num_edges;
                    self.critical_path_us = info
                        .critical_path2
                        .iter()
                        .map(|e| {
                            e.duration
                                .as_ref()
                                .map(|d| {
                                    (d.seconds as u64).saturating_mul(1_000_000)
                                        + (d.nanos as u64 / 1_000)
                                })
                                .unwrap_or(0)
                        })
                        .sum();
                }
                Some(data::instant_event::Data::Snapshot(snap)) => {
                    if let Some(t) = ts_us {
                        let net: (u64, u64) = snap.network_interface_stats.values().fold(
                            (0u64, 0u64),
                            |(tx, rx), stats| {
                                (
                                    tx.saturating_add(stats.tx_bytes),
                                    rx.saturating_add(stats.rx_bytes),
                                )
                            },
                        );
                        self.snapshots.push(SnapshotSample {
                            ts_us: t,
                            slug_rss_bytes: snap.slug_rss.unwrap_or(0),
                            slug_cpu_us: snap
                                .slug_user_cpu_us
                                .saturating_add(snap.slug_system_cpu_us),
                            host_cpu_ms: snap
                                .host_cpu_usage_system_ms
                                .unwrap_or(0)
                                .saturating_add(snap.host_cpu_usage_user_ms.unwrap_or(0)),
                            load1: snap
                                .unix_system_stats
                                .as_ref()
                                .map(|s| s.load1)
                                .unwrap_or(0.0),
                            net_tx_bytes: net.0,
                            net_rx_bytes: net.1,
                        });
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// `source` is the original slug `BuckEvent` the BEP event was
    /// derived from. The chrome trace's `tid` is sourced from there
    /// (`ActionExecutionEnd.local_thread_id`) — BEP's `Action` event
    /// has no thread-id field so we read it off the slug side.
    pub fn observe(&mut self, source: Option<&data::BuckEvent>, event: &bep::BuildEvent) {
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
                    let action_end = source
                        .and_then(|e| e.data.as_ref())
                        .and_then(|d| match d {
                            data::buck_event::Data::SpanEnd(se) => se.data.as_ref(),
                            _ => None,
                        })
                        .and_then(|sed| match sed {
                            data::span_end_event::Data::ActionExecution(ae) => Some(ae),
                            _ => None,
                        });
                    let tid = action_end.map(|ae| ae.local_thread_id).unwrap_or(0);
                    let thread_name = action_end
                        .map(|ae| ae.local_thread_name.clone())
                        .unwrap_or_default();

                    // Track local action-cache hits separately. Bazel's
                    // `ActionSummary.actions_executed` and per-mnemonic
                    // `ActionData.actions_executed` explicitly *exclude*
                    // local action-cache hits (their docs: "includes
                    // remote cache hits but excludes local action cache
                    // hits") — those go in `action_cache_statistics`.
                    let exec_kind = action_end.map(|ae| ae.execution_kind).unwrap_or(0);
                    let is_local_action_cache_hit =
                        exec_kind == data::ActionExecutionKind::ActionCache as i32;
                    if is_local_action_cache_hit {
                        self.local_action_cache_hits =
                            self.local_action_cache_hits.saturating_add(1);
                    } else {
                        self.local_action_cache_misses =
                            self.local_action_cache_misses.saturating_add(1);
                        let entry = self.actions_by_mnemonic.entry(mnemonic.clone()).or_insert(
                            MnemonicStats {
                                actions_executed: 0,
                                first_started_ms: i64::MAX,
                                last_ended_ms: 0,
                            },
                        );
                        entry.actions_executed += 1;
                        entry.first_started_ms = entry.first_started_ms.min(start / 1_000);
                        entry.last_ended_ms = entry.last_ended_ms.max(end / 1_000);
                    }

                    // Track per-executor-kind counts for Bazel's
                    // `ActionSummary.runner_count` chips. Names match
                    // Bazel's display strings ("local", "remote", …).
                    // Local-cache hits get their own bucket so the
                    // chip totals reconcile to the cli's
                    // `Commands: N (cached: A, remote: B, local: C)`.
                    let kind_name = match data::ActionExecutionKind::try_from(exec_kind)
                        .unwrap_or(data::ActionExecutionKind::NotSet)
                    {
                        data::ActionExecutionKind::Local => "local",
                        data::ActionExecutionKind::Remote => "remote",
                        data::ActionExecutionKind::ActionCache => "local-cache",
                        data::ActionExecutionKind::Simple => "internal",
                        data::ActionExecutionKind::Deferred => "deferred",
                        data::ActionExecutionKind::LocalDepFile => "local-dep-file-cache",
                        data::ActionExecutionKind::LocalWorker => "worker",
                        data::ActionExecutionKind::RemoteDepFileCache => "remote-dep-file-cache",
                        _ => "unknown",
                    };
                    *self.runner_counts.entry(kind_name.to_owned()).or_insert(0) += 1;

                    // Sum action wall durations for `cpu_time_in_ms`.
                    // Per-action OS-level CPU time isn't tracked at
                    // this layer, so wall-sum is the best proxy.
                    self.total_action_wall_us = self.total_action_wall_us.saturating_add(dur);
                    // BB's Timing tab filters action events by `cat
                    // == "action processing"` — that's the literal
                    // string Bazel writes. Putting the mnemonic in
                    // `cat` (as slug did before) made BB's parser
                    // ignore everything as "unknown event," so the
                    // tab stayed at "Build is in progress…" even
                    // when our trace was structurally well-formed.
                    // The mnemonic moves to `args.mnemonic`,
                    // matching Bazel exactly.
                    self.action_traces.push(TraceEvent {
                        name,
                        category: "action processing".to_owned(),
                        mnemonic,
                        ts_us: start,
                        dur_us: dur,
                        tid,
                        thread_name,
                        execution_kind: exec_kind,
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
    /// Returns the gzipped Chrome-trace bytes for the build, suitable
    /// for upload as `command.profile.gz`. `None` when no actions were
    /// observed (so we'd just be sending an empty profile).
    pub fn build_profile_gz(&self) -> Option<Vec<u8>> {
        if self.action_traces.is_empty() {
            return None;
        }
        let json = self.build_profile_json();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        use std::io::Write;
        if gz.write_all(json.as_bytes()).is_err() {
            return None;
        }
        gz.finish().ok()
    }

    /// JSON form of the trace, matching Bazel's `JsonTraceFileWriter`
    /// output shape so BuildBuddy's Timing tab parser accepts it.
    /// Reverse-engineered from `bazel build … --profile=…` output:
    ///
    ///   ```text
    ///   {"otherData":{"bazel_version":"…","build_id":"…",…},"traceEvents":[
    ///       {"name":"thread_name","ph":"M","pid":1,"tid":0,"args":{"name":"Critical Path"}},
    ///       {"name":"thread_sort_index","ph":"M","pid":1,"tid":0,"args":{"sort_index":0}},
    ///       {"cat":"…","name":"…","ph":"X","ts":<us>,"dur":<us>,"pid":1,"tid":0},
    ///       …
    ///     ]
    ///   }
    ///   ```
    ///
    /// Notable: action `X` events do NOT include `args` when there's
    /// nothing to put in them (slug previously emitted `"args":{}`),
    /// the `tid` is always set, and Bazel pretty-prints with newlines
    /// between events.
    ///
    /// `otherData` shape mirrors Bazel's exactly — including
    /// `bazel_version`, `build_id`, `output_base`, `date`, and
    /// `profile_start_ts`. Earlier slug versions only emitted the
    /// first three; BuildBuddy's Timing tab tolerated the abbreviated
    /// form on small invocations (a few hundred actions) but stayed
    /// stuck at "Build is in progress…" for clang-scale builds (~4500
    /// actions). Aligning with Bazel's full schema fixes the large
    /// case. `bazel_version` reads `release 8.0.0-slug` rather than a
    /// bare `slug` because BB parses the leading `release ` token.
    ///
    /// Tid assignment matches Bazel: each event's `tid` is the
    /// `local_thread_id` captured at action-end time — a process-wide
    /// monotonic counter incremented per OS thread, the same scheme
    /// `java.lang.Thread.getId()` uses. Each tokio worker that polls
    /// an action's completion gets one stable id, and parallel actions
    /// fan out across distinct tids. For remote actions the tid is
    /// the *submitting/awaiting* worker, not the BB executor that ran
    /// the compute — Bazel does the same. `tid=0` is reserved for the
    /// Critical Path metadata lane; events with `tid==0` (i.e. trace
    /// events from before `local_thread_id` plumbing existed, or test
    /// fixtures) are bumped to `tid=1` so they don't collide with
    /// Critical Path.
    pub fn build_profile_json(&self) -> String {
        let mut json = String::with_capacity(512 + self.action_traces.len() * 160);
        json.push_str(r#"{"otherData":{"#);
        json.push_str(&format!(
            r#""bazel_version":"release 8.0.0-slug","build_id":{build_id},"output_base":"","date":{date},"profile_start_ts":{ts}"#,
            build_id = json_string(&self.build_id),
            date = json_string(&self.profile_start_iso),
            ts = self.profile_start_ms,
        ));
        json.push_str(r#"}"#);
        json.push_str(
            r#","traceEvents":[
"#,
        );
        json.push_str(
            r#"    {"name":"thread_name","ph":"M","pid":1,"tid":0,"args":{"name":"Critical Path"}}"#,
        );
        json.push_str(
            ",\n    {\"name\":\"thread_sort_index\",\"ph\":\"M\",\"pid\":1,\"tid\":0,\"args\":{\"sort_index\":0}}",
        );

        let origin_us = self
            .action_traces
            .iter()
            .map(|e| e.ts_us)
            .min()
            .unwrap_or(0);

        // Bump the "not captured" sentinel (0) up to 1 so events don't
        // collide with the Critical Path lane.
        let event_tid = |e: &TraceEvent| -> u64 { if e.tid == 0 { 1 } else { e.tid } };

        // Emit a `thread_name` / `thread_sort_index` metadata pair for
        // every distinct tid that appears on an action event, in
        // ascending order. Without these declarations BB renders the
        // tid as a numeric label (e.g. `42`). The lane label prefers
        // the OS thread's actual name (e.g. `slug-rt-3` from tokio's
        // `thread_name_fn`) — that mirrors Bazel's
        // `Thread.currentThread().getName()` shape (e.g.
        // `skyframe-evaluator-N`). Falls back to `Worker N` when no
        // name was captured (test fixtures, callers without a tokio
        // runtime).
        let mut tid_to_name: std::collections::BTreeMap<u64, String> =
            std::collections::BTreeMap::new();
        for e in &self.action_traces {
            let tid = event_tid(e);
            let entry = tid_to_name.entry(tid).or_default();
            // Keep the first non-empty name we see for this tid.
            // Different actions on the same OS thread (which work-
            // stealing is allowed to do) all share that thread's
            // name, so any non-empty value is correct.
            if entry.is_empty() && !e.thread_name.is_empty() {
                *entry = e.thread_name.clone();
            }
        }
        for (tid, name) in &tid_to_name {
            let label = if name.is_empty() {
                format!("Worker {tid}")
            } else {
                name.clone()
            };
            json.push_str(&format!(
                ",\n    {{\"name\":\"thread_name\",\"ph\":\"M\",\"pid\":1,\"tid\":{tid},\"args\":{{\"name\":{label}}}}}",
                tid = tid,
                label = json_string(&label),
            ));
            json.push_str(&format!(
                ",\n    {{\"name\":\"thread_sort_index\",\"ph\":\"M\",\"pid\":1,\"tid\":{tid},\"args\":{{\"sort_index\":{tid}}}}}",
                tid = tid,
            ));
        }

        for e in &self.action_traces {
            json.push_str(",\n    ");
            let ts = e.ts_us.saturating_sub(origin_us);
            let tid = event_tid(e);
            // Field order matches Bazel's `TaskData.writeTraceData`:
            // `cat`, `name`, `ph`, `ts`, `dur`, `pid`, then `args`
            // (when present), then `tid`. `args.mnemonic` carries
            // the action mnemonic — BB's Timing tab reads it for the
            // per-event tooltip and to color the timeline by mnemonic.
            if e.mnemonic.is_empty() {
                json.push_str(&format!(
                    "{{\"cat\":{cat},\"name\":{name},\"ph\":\"X\",\"ts\":{ts},\"dur\":{dur},\"pid\":1,\"tid\":{tid}}}",
                    name = json_string(&e.name),
                    cat = json_string(&e.category),
                    ts = ts,
                    dur = e.dur_us,
                    tid = tid,
                ));
            } else {
                json.push_str(&format!(
                    "{{\"cat\":{cat},\"name\":{name},\"ph\":\"X\",\"ts\":{ts},\"dur\":{dur},\"pid\":1,\"args\":{{\"mnemonic\":{mnemonic}}},\"tid\":{tid}}}",
                    name = json_string(&e.name),
                    cat = json_string(&e.category),
                    ts = ts,
                    dur = e.dur_us,
                    mnemonic = json_string(&e.mnemonic),
                    tid = tid,
                ));
            }
            // Companion event: BB's Timing tab "Execution Breakdown"
            // pie chart sums durations by *specific* event names —
            // `subprocess.run` / `execute remotely` / `check cache hit`.
            // Our descriptive `<mnemonic> <label>` name doesn't match,
            // so without these companions the breakdown card stays
            // hidden (its `phaseData`/`executionData` arrays filter
            // out zero-value entries and the entire card returns nothing
            // to render). Bazel emits the same shape: one descriptive
            // action event for the timeline + nested `subprocess.run`
            // events for the breakdown sums.
            let companion_name = match data::ActionExecutionKind::try_from(e.execution_kind)
                .unwrap_or(data::ActionExecutionKind::NotSet)
            {
                data::ActionExecutionKind::Local | data::ActionExecutionKind::LocalWorker => {
                    Some("subprocess.run")
                }
                data::ActionExecutionKind::Remote => Some("execute remotely"),
                data::ActionExecutionKind::ActionCache
                | data::ActionExecutionKind::LocalDepFile
                | data::ActionExecutionKind::RemoteDepFileCache => Some("check cache hit"),
                _ => None,
            };
            if let Some(name) = companion_name {
                json.push_str(&format!(
                    ",\n    {{\"cat\":\"general information\",\"name\":\"{name}\",\"ph\":\"X\",\"ts\":{ts},\"dur\":{dur},\"pid\":1,\"tid\":{tid}}}",
                    name = name,
                    ts = ts,
                    dur = e.dur_us,
                    tid = tid,
                ));
            }
        }

        // Synthetic `buildTargets` event covering the entire command
        // span. BB's Timing tab "Phase breakdown" pie computes
        // `building = buildTargets - runAnalysisPhase - evaluateTargetPatterns`
        // and renders only when at least one phase has positive
        // duration. With analysis + evaluation phases unmeasured (slug
        // doesn't yet plumb phase markers through to BES), `buildTargets`
        // alone yields a single "Execution" slice spanning the whole
        // build — enough to make the card render.
        if self.command_end_us > self.command_start_us && self.command_start_us > 0 {
            let ts = self.command_start_us.saturating_sub(origin_us);
            let dur = self.command_end_us - self.command_start_us;
            json.push_str(&format!(
                ",\n    {{\"cat\":\"general information\",\"name\":\"buildTargets\",\"ph\":\"X\",\"ts\":{ts},\"dur\":{dur},\"pid\":1,\"tid\":1}}",
                ts = ts,
                dur = dur,
            ));
        }

        // Counter (`ph: "C"`) events for the timeseries panel BB
        // renders between the flamegraph and the Timing Breakdown
        // card. BB's `TIME_SERIES_METADATA` table (in
        // `app/trace/trace_events.ts`) hardcodes the event names + arg
        // keys that get rendered as line plots: "CPU usage (Bazel)" /
        // args.cpu, "Memory usage (Bazel)" / args.memory, etc. The
        // panel filters out empty sections, so without these events
        // the panel disappears entirely. Rate-style series (CPU
        // cores, network Mbps) need delta-vs-prev-snapshot; level
        // series (memory, load) emit the value directly.
        for (i, snap) in self.snapshots.iter().enumerate() {
            let ts = snap.ts_us.saturating_sub(origin_us);
            // Memory in MB — Bazel's plot expects megabytes.
            let mem_mb = snap.slug_rss_bytes as f64 / 1_000_000.0;
            json.push_str(&format!(
                ",\n    {{\"name\":\"Memory usage (Bazel)\",\"ph\":\"C\",\"ts\":{ts},\"pid\":1,\"tid\":1,\"args\":{{\"memory\":{mem:.2}}}}}",
                ts = ts,
                mem = mem_mb,
            ));
            json.push_str(&format!(
                ",\n    {{\"name\":\"System load average\",\"ph\":\"C\",\"ts\":{ts},\"pid\":1,\"tid\":1,\"args\":{{\"load\":{load:.3}}}}}",
                ts = ts,
                load = snap.load1,
            ));
            // Rate series — need previous snapshot for the delta.
            if i == 0 {
                continue;
            }
            let prev = &self.snapshots[i - 1];
            let dt_us = (snap.ts_us - prev.ts_us).max(1) as f64;
            let dt_s = dt_us / 1_000_000.0;
            // CPU usage in cores: dt_us of cpu over dt_us of wall.
            let cpu_slug_cores = (snap.slug_cpu_us.saturating_sub(prev.slug_cpu_us)) as f64 / dt_us;
            let cpu_host_cores =
                (snap.host_cpu_ms.saturating_sub(prev.host_cpu_ms)) as f64 * 1_000.0 / dt_us;
            json.push_str(&format!(
                ",\n    {{\"name\":\"CPU usage (Bazel)\",\"ph\":\"C\",\"ts\":{ts},\"pid\":1,\"tid\":1,\"args\":{{\"cpu\":{cpu:.3}}}}}",
                ts = ts,
                cpu = cpu_slug_cores,
            ));
            json.push_str(&format!(
                ",\n    {{\"name\":\"CPU usage (total)\",\"ph\":\"C\",\"ts\":{ts},\"pid\":1,\"tid\":1,\"args\":{{\"system cpu\":{cpu:.3}}}}}",
                ts = ts,
                cpu = cpu_host_cores,
            ));
            // Network throughput in Mbps: bytes*8 / 1e6 / sec.
            let net_up_mbps = (snap.net_tx_bytes.saturating_sub(prev.net_tx_bytes)) as f64 * 8.0
                / 1_000_000.0
                / dt_s;
            let net_dn_mbps = (snap.net_rx_bytes.saturating_sub(prev.net_rx_bytes)) as f64 * 8.0
                / 1_000_000.0
                / dt_s;
            json.push_str(&format!(
                ",\n    {{\"name\":\"Network Up usage (total)\",\"ph\":\"C\",\"ts\":{ts},\"pid\":1,\"tid\":1,\"args\":{{\"system network up (Mbps)\":{up:.3}}}}}",
                ts = ts,
                up = net_up_mbps,
            ));
            json.push_str(&format!(
                ",\n    {{\"name\":\"Network Down usage (total)\",\"ph\":\"C\",\"ts\":{ts},\"pid\":1,\"tid\":1,\"args\":{{\"system network down (Mbps)\":{dn:.3}}}}}",
                ts = ts,
                dn = net_dn_mbps,
            ));
        }
        json.push_str("\n  ]\n}");
        json
    }

    /// Build a `BuildToolLogs` event whose `command.profile.gz` entry
    /// references `gz_uri` (a `bytestream://…/blobs/<hash>/<size>`
    /// URL produced by uploading the bytes from `build_profile_gz()`
    /// to BB's CAS). BuildBuddy's Timing tab requires the URI form;
    /// inline `contents` does not light it up.
    pub fn build_tool_logs_event_with_uri(&self, gz_uri: String) -> Option<bep::BuildEvent> {
        use bep::build_event_id as beid;

        if self.action_traces.is_empty() {
            return None;
        }
        // `File.length` shape isn't actually defined by the BEP proto
        // for the `uri` case — Bazel sets it to whatever it has handy
        // and BuildBuddy's frontend mostly ignores it (the bytestream
        // resource path encodes the compressed size, which is
        // authoritative). Leaving it at `-1` prevents a stale
        // mismatch between this field and what the URI carries.
        let length: i64 = -1;
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
                        length,
                        file: Some(bep::file::File::Uri(gz_uri)),
                    }],
                },
            )),
        })
    }

    pub fn build_tool_logs_event(&self) -> Option<bep::BuildEvent> {
        use bep::build_event_id as beid;

        if self.action_traces.is_empty() {
            return None;
        }

        // Mirrors `build_profile_json()` so the inline-bytes BEP file
        // path emitted by `bep_file_sink` carries the same Bazel-shaped
        // JSON the bytestream upload does. BuildBuddy's Timing tab
        // requires the URI form (separate code path); this one is
        // mostly used for local debugging via `--build_event_binary_file`.
        let json = self.build_profile_json();
        let json_bytes = json.into_bytes();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        use std::io::Write;
        if gz.write_all(&json_bytes).is_err() {
            return None;
        }
        let gz_bytes = match gz.finish() {
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
                    log: vec![
                        bep::File {
                            path_prefix: Vec::new(),
                            name: "command.profile.gz".to_owned(),
                            digest: String::new(),
                            length: gz_bytes.len() as i64,
                            file: Some(bep::file::File::Contents(gz_bytes)),
                        },
                        bep::File {
                            path_prefix: Vec::new(),
                            name: "command.profile.json".to_owned(),
                            digest: String::new(),
                            length: json_bytes.len() as i64,
                            file: Some(bep::file::File::Contents(json_bytes)),
                        },
                    ],
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

        // Per-mnemonic action breakdown — Bazel emits one `ActionData`
        // entry per mnemonic, sorted descending by `actions_executed`.
        // BB's invocation page renders this as the action-count
        // summary table. Mnemonics that only saw local-cache hits are
        // skipped (they don't count as `actions_executed`, and Bazel
        // only emits non-zero rows here).
        let mut action_data: Vec<_> = self
            .actions_by_mnemonic
            .iter()
            .map(
                |(mnemonic, stats)| bep::build_metrics::action_summary::ActionData {
                    mnemonic: mnemonic.clone(),
                    actions_executed: stats.actions_executed,
                    first_started_ms: if stats.first_started_ms == i64::MAX {
                        0
                    } else {
                        stats.first_started_ms
                    },
                    last_ended_ms: stats.last_ended_ms,
                    system_time: None,
                    user_time: None,
                    actions_created: stats.actions_executed,
                },
            )
            .collect();
        action_data.sort_by(|a, b| b.actions_executed.cmp(&a.actions_executed));

        // `executed_total` follows Bazel's "executed actions excluding
        // local-cache hits" definition. The on-screen "Action Count"
        // chip on BB shows this directly.
        let executed_total: i64 = self
            .actions_by_mnemonic
            .values()
            .map(|s| s.actions_executed)
            .sum();

        // ActionCacheStatistics drives the "Local Action Cache Hits"
        // panel. Only `hits` and `misses` are populated; the other
        // fields (size_in_bytes, save_time_in_ms, miss_details) are
        // bazel internals we don't track.
        let action_cache_statistics =
            if self.local_action_cache_hits + self.local_action_cache_misses > 0 {
                Some(crate::blaze::ActionCacheStatistics {
                    size_in_bytes: 0,
                    save_time_in_ms: 0,
                    hits: self.local_action_cache_hits,
                    misses: self.local_action_cache_misses,
                    miss_details: Vec::new(),
                    load_time_in_ms: 0,
                    cache_check_semaphore_wait_time_in_ms: 0,
                })
            } else {
                None
            };

        // Wall-clock spans. Prefer the top-level `Command` span when
        // we observed both ends — that gives us a meaningful
        // wall_time even when the build is fully cached / has no
        // action events. Otherwise fall back to the action-span
        // estimate. `analysis_phase_time` stays 0 for now (slug
        // doesn't surface analysis-phase timing into BES yet); BB's
        // "Timing Breakdown" pie chart lumps everything into
        // Execution as a result.
        let wall_us: i64 =
            if self.command_end_us > self.command_start_us && self.command_start_us > 0 {
                self.command_end_us - self.command_start_us
            } else if self.last_action_ended_ms > self.first_action_started_ms
                && self.first_action_started_ms > 0
            {
                (self.last_action_ended_ms - self.first_action_started_ms) * 1_000
            } else {
                0
            };
        let exec_us: i64 = if self.last_action_ended_ms > self.first_action_started_ms
            && self.first_action_started_ms > 0
        {
            (self.last_action_ended_ms - self.first_action_started_ms) * 1_000
        } else {
            0
        };
        let critical_path_time = if self.critical_path_us > 0 {
            // Saturating cast: critical path durations measured in us
            // fit comfortably in i32 seconds for any reasonable build,
            // but bound the conversion just in case.
            let secs = (self.critical_path_us / 1_000_000) as i64;
            let nanos = ((self.critical_path_us % 1_000_000) * 1_000) as i32;
            Some(prost_types::Duration {
                seconds: secs,
                nanos,
            })
        } else {
            None
        };
        let timing_metrics = if wall_us > 0 || exec_us > 0 {
            // `cpu_time_in_ms` here is the sum of action wall
            // durations (a rough proxy — slug doesn't track per-action
            // OS-level CPU time). For a heavily parallel build this
            // exceeds wall_time_in_ms, matching Bazel's behaviour
            // where summed CPU time also exceeds wall on multi-core
            // builds.
            Some(bep::build_metrics::TimingMetrics {
                cpu_time_in_ms: self.total_action_wall_us / 1_000,
                wall_time_in_ms: wall_us / 1_000,
                analysis_phase_time_in_ms: 0,
                execution_phase_time_in_ms: exec_us / 1_000,
                actions_execution_start_in_ms: 0,
                critical_path_time,
            })
        } else {
            None
        };

        // Build graph metrics (action graph node/edge counts) come
        // from the terminal `BuildGraphExecutionInfo` instant event.
        // BB's invocation page surfaces these in the build-graph
        // summary section.
        let build_graph_metrics = if self.graph_num_nodes > 0 || self.graph_num_edges > 0 {
            // Most BuildGraphMetrics fields are SkyFunction-specific
            // (Bazel evaluator internals); slug doesn't expose
            // equivalents. Populate the universal node/action counts
            // from BuildGraphExecutionInfo and leave the rest at
            // their proto defaults so the BB invocation page still
            // gets its build-graph summary chip.
            Some(bep::build_metrics::BuildGraphMetrics {
                action_lookup_value_count: self.graph_num_nodes as i32,
                action_lookup_value_count_not_including_aspects: self.graph_num_nodes as i32,
                action_count: self.graph_num_nodes as i32,
                action_count_not_including_aspects: self.graph_num_nodes as i32,
                input_file_configured_target_count: 0,
                output_file_configured_target_count: 0,
                other_configured_target_count: 0,
                output_artifact_count: 0,
                post_invocation_skyframe_node_count: self.graph_num_nodes as i32,
                dirtied_values: Vec::new(),
                changed_values: Vec::new(),
                evaluated_values: Vec::new(),
                built_values: Vec::new(),
                cleaned_values: Vec::new(),
                aspect: Vec::new(),
                rule_class: Vec::new(),
            })
        } else {
            None
        };

        // Bazel emits one `RunnerCount` row per executor kind
        // (e.g. `local`, `remote`, `worker`). BB renders these as
        // chips on the invocation page so the user can see the
        // local/remote/cache split at a glance. Sorted descending
        // by count so the heaviest bucket comes first.
        let mut runner_count: Vec<_> = self
            .runner_counts
            .iter()
            .map(
                |(name, count)| bep::build_metrics::action_summary::RunnerCount {
                    name: name.clone(),
                    count: *count,
                    exec_kind: name.clone(),
                },
            )
            .collect();
        runner_count.sort_by(|a, b| b.count.cmp(&a.count));

        tracing::info!(
            "BuildMetrics emit: actions_executed={} mnemonics={} runner_kinds={} \
             cache_hits={} cache_misses={} graph_nodes={} crit_path_us={} \
             wall_us={} cmd_span={}..{}",
            executed_total,
            action_data.len(),
            runner_count.len(),
            self.local_action_cache_hits,
            self.local_action_cache_misses,
            self.graph_num_nodes,
            self.critical_path_us,
            wall_us,
            self.command_start_us,
            self.command_end_us,
        );
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
                    actions_executed: executed_total,
                    action_data,
                    remote_cache_hits: 0,
                    runner_count,
                    action_cache_statistics,
                }),
                memory_metrics: None,
                target_metrics: Some(bep::build_metrics::TargetMetrics {
                    targets_loaded: self.targets_configured,
                    targets_configured: self.targets_configured,
                    targets_configured_not_including_aspects: self.targets_configured,
                }),
                package_metrics: None,
                timing_metrics,
                cumulative_metrics: None,
                artifact_metrics: None,
                build_graph_metrics,
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

/// Strip the root cell name prefix from a target pattern. Slug stores
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
/// timing graphs where slug records `wall_time` only.
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

/// Render the owning target label from an `ActionKey` as a Bazel-style
/// `//pkg:name` string (or empty when the key has no `target_label`,
/// e.g. anon targets / BXL keys / local-resource-setup actions).
fn action_owner_label(key: &data::ActionKey) -> String {
    use data::action_key::Owner;
    match key.owner.as_ref() {
        Some(Owner::TargetLabel(t))
        | Some(Owner::TestTargetLabel(t))
        | Some(Owner::LocalResourceSetup(t)) => render_configured_label(t),
        _ => String::new(),
    }
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
/// events that don't have a natural Slug counterpart (e.g., `Aborted`).
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

/// Returns `(iso_8601_utc, millis_since_unix_epoch)` for "now". Both
/// forms feed Bazel-shape `otherData` fields in the chrome trace.
/// ISO formatting is RFC 3339 in UTC with nanosecond precision and the
/// `Z` suffix, mirroring Bazel's `Instant.toString()` output (e.g.
/// `2026-04-27T23:43:25.675932295Z`).
fn current_time_pair() -> (String, i64) {
    let now = chrono::Utc::now();
    // `%9f` = 9-digit fractional seconds (nanoseconds), matching the
    // shape of Bazel's `java.time.Instant.toString()`. `chrono`'s
    // built-in `to_rfc3339()` uses microsecond precision and would
    // fall short of Bazel's 9-digit form.
    let iso = now.format("%Y-%m-%dT%H:%M:%S%.9fZ").to_string();
    let ms = now.timestamp_millis();
    (iso, ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `BepStreamState::with_invocation_id` populates the chrome trace's
    /// `otherData.build_id`. BuildBuddy's Timing tab cross-references this
    /// with the BES stream's invocation_id; for large invocations
    /// (~thousands of actions) a mismatch leaves the tab stuck in
    /// "Build is in progress…". This test pins the substring so a future
    /// refactor can't silently regress to the hardcoded `"slug"` literal.
    #[test]
    fn chrome_trace_build_id_uses_invocation_id() {
        let invocation_id = "0d2d7a3f-eae9-4f10-a204-2cbc34961619".to_owned();
        let mut state = BepStreamState::with_invocation_id(invocation_id.clone());
        // Push at least one action so build_profile_json() actually emits.
        state.action_traces.push(TraceEvent {
            name: "test".to_owned(),
            category: "Action".to_owned(),
            mnemonic: "Action".to_owned(),
            ts_us: 0,
            dur_us: 1,
            tid: 7,
            thread_name: String::new(),
            execution_kind: 0,
        });
        let json = state.build_profile_json();
        assert!(
            json.contains(&format!(r#""build_id":"{invocation_id}""#)),
            "trace JSON must embed the invocation_id as build_id; got: {json}"
        );
        assert!(
            !json.contains(r#""build_id":"slug""#),
            "trace JSON must NOT use the hardcoded 'slug' literal; got: {json}"
        );
    }

    /// `BepStreamState::new()` (used in non-BES contexts like file-only
    /// BEP sinks) keeps the legacy `"slug"` placeholder. This is fine —
    /// no BB cross-correlation is happening, and the field is just
    /// reference metadata.
    #[test]
    fn chrome_trace_default_build_id_is_slug() {
        let mut state = BepStreamState::new();
        state.action_traces.push(TraceEvent {
            name: "test".to_owned(),
            category: "Action".to_owned(),
            mnemonic: "Action".to_owned(),
            ts_us: 0,
            dur_us: 1,
            tid: 1,
            thread_name: String::new(),
            execution_kind: 0,
        });
        let json = state.build_profile_json();
        assert!(
            json.contains(r#""build_id":"slug""#),
            "default build_id must remain 'slug'; got: {json}"
        );
    }

    /// Pin every Bazel-shape field BB's Timing parser checks for at
    /// scale. Earlier slug versions emitted only `bazel_version` /
    /// `build_id` / `output_base`; BB tolerated this for small builds
    /// (~hundreds of actions) but the Timing tab stayed stuck at
    /// "Build is in progress…" for clang-scale invocations. Aligning
    /// `otherData` with Bazel's full schema (`bazel_version` starts
    /// with `release `, plus `date` and `profile_start_ts`) and
    /// fanning action events across distinct worker tids (Bazel's
    /// trace uses ~hundreds of distinct tids for parallel work)
    /// restored the large-build path.
    #[test]
    fn chrome_trace_matches_bazel_schema() {
        let mut state = BepStreamState::with_invocation_id("test-id".to_owned());
        state.action_traces.push(TraceEvent {
            name: "Compiling foo.cpp".to_owned(),
            category: "action processing".to_owned(),
            mnemonic: "c_compile".to_owned(),
            ts_us: 1_000,
            dur_us: 500,
            tid: 7,
            thread_name: String::new(),
            execution_kind: 0,
        });
        let json = state.build_profile_json();

        // bazel_version begins with "release " — BB parses the leading
        // token. Bare "slug" caused the parser to bail at scale.
        assert!(
            json.contains(r#""bazel_version":"release "#),
            "bazel_version must start with 'release '; got: {json}"
        );
        // date / profile_start_ts are required for BB's anchoring.
        assert!(
            json.contains(r#""date":""#),
            "trace must include otherData.date; got: {json}"
        );
        assert!(
            json.contains(r#""profile_start_ts":"#),
            "trace must include otherData.profile_start_ts; got: {json}"
        );
        // Captured tid (7) flows through unchanged.
        assert!(
            json.contains(r#""args":{"name":"Worker 7"}"#),
            "trace must declare Worker 7 thread_name; got: {json}"
        );
        assert!(
            json.contains(r#""tid":7}"#),
            "action event must keep tid=7; got: {json}"
        );
        // Action events must use the literal `cat: "action processing"`
        // (Bazel's convention) and carry the mnemonic in args.
        assert!(
            json.contains(r#""cat":"action processing""#),
            "action event must use cat=\"action processing\"; got: {json}"
        );
        assert!(
            json.contains(r#""args":{"mnemonic":"c_compile"}"#),
            "action event must carry mnemonic in args; got: {json}"
        );
    }

    /// Each distinct captured tid produces exactly one
    /// `thread_name`/`thread_sort_index` metadata pair. Two events on
    /// the same tid (same worker thread polled both completions) share
    /// the same Worker N row; events on different tids each declare
    /// their own row. This mirrors Bazel's behavior where one JVM
    /// thread can handle many sequential actions.
    #[test]
    fn chrome_trace_distinct_tids_declare_distinct_workers() {
        let mut state = BepStreamState::with_invocation_id("test-id".to_owned());
        state.action_traces.push(TraceEvent {
            name: "a".to_owned(),
            category: "action processing".to_owned(),
            mnemonic: "c".to_owned(),
            ts_us: 1_000,
            dur_us: 500,
            tid: 3,
            thread_name: String::new(),
            execution_kind: 0,
        });
        state.action_traces.push(TraceEvent {
            name: "b".to_owned(),
            category: "action processing".to_owned(),
            mnemonic: "c".to_owned(),
            ts_us: 2_000,
            dur_us: 500,
            tid: 5,
            thread_name: String::new(),
            execution_kind: 0,
        });
        // Same tid as the first event — should not produce a second
        // Worker 3 declaration.
        state.action_traces.push(TraceEvent {
            name: "c".to_owned(),
            category: "action processing".to_owned(),
            mnemonic: "c".to_owned(),
            ts_us: 3_000,
            dur_us: 500,
            tid: 3,
            thread_name: String::new(),
            execution_kind: 0,
        });
        let json = state.build_profile_json();

        assert!(
            json.contains(r#""args":{"name":"Worker 3"}"#),
            "Worker 3 must be declared; got: {json}"
        );
        assert!(
            json.contains(r#""args":{"name":"Worker 5"}"#),
            "Worker 5 must be declared; got: {json}"
        );
        // Worker 3 declared exactly once even though two events share it.
        assert_eq!(
            json.matches(r#""args":{"name":"Worker 3"}"#).count(),
            1,
            "Worker 3 must be declared exactly once for two events on tid=3; got: {json}"
        );
    }

    /// When a `thread_name` is captured (e.g. tokio's
    /// `thread_name_fn` set the worker's name to `slug-rt-3`), the
    /// chrome trace lane label uses that name verbatim instead of the
    /// `Worker N` placeholder. Mirrors Bazel's
    /// `Thread.currentThread().getName()` shape (e.g.
    /// `skyframe-evaluator-N`).
    #[test]
    fn chrome_trace_uses_captured_thread_name_for_lane_label() {
        let mut state = BepStreamState::with_invocation_id("test-id".to_owned());
        state.action_traces.push(TraceEvent {
            name: "a".to_owned(),
            category: "action processing".to_owned(),
            mnemonic: "c_compile".to_owned(),
            ts_us: 1_000,
            dur_us: 500,
            tid: 3,
            thread_name: "slug-rt-3".to_owned(),
            execution_kind: 0,
        });
        let json = state.build_profile_json();

        // Lane label is the captured OS thread name, not "Worker 3".
        assert!(
            json.contains(r#""args":{"name":"slug-rt-3"}"#),
            "lane label must use captured thread name; got: {json}"
        );
        assert!(
            !json.contains(r#""args":{"name":"Worker 3"}"#),
            "lane label must NOT fall back to Worker 3 when name is captured; got: {json}"
        );
    }

    /// Events with the "not captured" sentinel (`tid=0`) get bumped up
    /// to `tid=1`. tid=0 is reserved for the Critical Path metadata
    /// lane; mixing action events there would break BB's renderer.
    #[test]
    fn chrome_trace_zero_tid_is_bumped_to_worker_1() {
        let mut state = BepStreamState::with_invocation_id("test-id".to_owned());
        state.action_traces.push(TraceEvent {
            name: "a".to_owned(),
            category: "action processing".to_owned(),
            mnemonic: "c".to_owned(),
            ts_us: 1_000,
            dur_us: 500,
            tid: 0,
            thread_name: String::new(),
            execution_kind: 0,
        });
        let json = state.build_profile_json();

        assert!(
            json.contains(r#""args":{"name":"Worker 1"}"#),
            "tid=0 must be bumped to Worker 1; got: {json}"
        );
        assert!(
            !json.contains(r#""ph":"X","ts":0,"dur":500,"pid":1,"tid":0"#),
            "action event must NOT keep tid=0 (reserved for Critical Path); got: {json}"
        );
    }

    /// `build_metrics_event` now emits per-mnemonic `ActionData`
    /// breakdown rows + `action_cache_statistics` + `timing_metrics`.
    /// BB's invocation page reads these for the action-count summary
    /// table, the "Local Action Cache Hits" panel, and the wall-time
    /// chip respectively.
    #[test]
    fn build_metrics_carries_per_mnemonic_breakdown_and_cache_stats() {
        use crate::build_event_stream::build_event::Payload;

        let mut state = BepStreamState::with_invocation_id("test-id".to_owned());
        // Two c_compile actions + one cpp_link action — non-cached.
        state.actions_by_mnemonic.insert(
            "c_compile".to_owned(),
            MnemonicStats {
                actions_executed: 2,
                first_started_ms: 1_000,
                last_ended_ms: 2_500,
            },
        );
        state.actions_by_mnemonic.insert(
            "cpp_link".to_owned(),
            MnemonicStats {
                actions_executed: 1,
                first_started_ms: 2_500,
                last_ended_ms: 3_000,
            },
        );
        state.local_action_cache_hits = 5;
        state.local_action_cache_misses = 3;
        state.first_action_started_ms = 1_000;
        state.last_action_ended_ms = 3_000;
        state.targets_configured = 7;

        let event = state.build_metrics_event();
        let payload = match event.payload {
            Some(Payload::BuildMetrics(m)) => m,
            other => panic!("expected BuildMetrics payload, got {other:?}"),
        };

        let summary = payload.action_summary.expect("ActionSummary");
        // Two distinct mnemonics → two ActionData rows, sorted descending
        // by actions_executed.
        assert_eq!(summary.action_data.len(), 2);
        assert_eq!(summary.action_data[0].mnemonic, "c_compile");
        assert_eq!(summary.action_data[0].actions_executed, 2);
        assert_eq!(summary.action_data[1].mnemonic, "cpp_link");
        assert_eq!(summary.action_data[1].actions_executed, 1);
        // Total executed = sum of per-mnemonic.
        assert_eq!(summary.actions_executed, 3);

        let stats = summary.action_cache_statistics.expect("cache stats");
        assert_eq!(stats.hits, 5);
        assert_eq!(stats.misses, 3);

        let timing = payload.timing_metrics.expect("timing metrics");
        assert_eq!(timing.wall_time_in_ms, 2_000);
        assert_eq!(timing.execution_phase_time_in_ms, 2_000);
    }
}
