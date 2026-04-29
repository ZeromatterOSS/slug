# Plan 18: BEP parity + remote endpoint support

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Follows Plan 16 (telemetry) and Plan 17 (optimizations). The event data
> this plan streams is the same data Plan 16 aggregates locally.

## Scope

Make Kuro speak Bazel's Build Event Protocol (BEP) and Build Event
Service (BES) well enough that third-party dashboards — BuildBuddy,
EngFlow, Trunk, BitRise, custom BES collectors — receive a Kuro
invocation as they would a Bazel invocation, with no dashboard-side
changes.

Goal invocation:

```
kuro build \
    --bes_backend=grpcs://remote.buildbuddy.io \
    --bes_results_url=https://app.buildbuddy.io/invocation/ \
    --build_event_json_file=./events.json \
    @llvm-project//clang:clang
```

Produces a BuildBuddy invocation page with target graph, action list,
critical path, cache stats — visually equivalent to Bazel's output for
the same target.

## Current State Analysis

Kuro's event infrastructure (confirmed in plan-1 research):

- `app/kuro_data/data.proto` — Kuro-native `BuckEvent` schema, ~2400
  lines, spans + instants + records.
- `app/kuro_events/src/dispatch.rs` — central pub-sub.
- `app/kuro_events/src/sink/*.rs` — channel, smart-truncate, remote
  (Scribe), tee, null. All file sinks write `.pb.zst`. No JSON sink.
- `app/kuro_event_log/src/{read,write}.rs` — zstd-framed proto varint
  stream.

What's missing vs Bazel:

- No `build_event_stream.proto` or `publish_build_event.proto`
  (Bazel 9's BEP schema).
- No BEP translation layer. `BuckEvent` is Kuro's dialect; BEP consumers
  expect Bazel's schema.
- No gRPC client streaming sink. The existing `RemoteEventSink` targets
  Meta's Scribe, which is unrelated.
- No `--build_event_json_file` / `--build_event_binary_file` /
  `--bes_*` flags.
- No invocation-metadata lift to BEP's `BuildMetadata` fields.
- No `Aborted` event emission on cancel (Ctrl+C / daemon shutdown) —
  BuildBuddy shows "still running" forever.

## Phases

### 18.1 Vendor BEP proto schema (DONE 2026-04-24)

Landed as `app/kuro_build_event_stream/` (Bazel 9.0.0rc1+834 + googleapis
`de157ca3`). One local modification: `analysis_cache_service_metadata_status.proto`
converted from `edition = "2023"` to `syntax = "proto3"` because prost-build
0.13 does not yet support proto editions; the file contains only an enum so
the change is wire-compatible. REv2 turned out to be unused by
`build_event_stream.proto` after checking the imports — not vendored.
Round-trip tests: `tests/roundtrip.rs`.


**Parity source.** Bazel 9.x source tree:
- `src/main/java/com/google/devtools/build/lib/buildeventstream/proto/build_event_stream.proto`
- `src/main/java/com/google/devtools/build/v1/publish_build_event.proto`
- Referenced types: `build/bazel/remote/execution/v2/*.proto` (REv2),
  `google/devtools/build/v1/*` (BES core), `google/api/*`.

Create `app/kuro_build_event_stream/` crate. Copy protos verbatim into
`proto/`. Wire `build.rs` to compile via `prost` (match existing kuro
crates' pattern). No schema modifications — we own the import,
upstream owns evolution.

Unit test: round-trip a sample BEP `BuildEvent` through the generated
Rust types.

---

### 18.2 Translation layer (DONE 2026-04-24 — extended for BB dashboard parity 2026-04-24)

`src/translate.rs` with `BuildEventContext` + per-event-kind functions. Covered:

- `CommandStart` → 8-event burst at invocation start: `Started`,
  `BuildMetadata`, `UnstructuredCommandLine`, three
  `StructuredCommandLine` views (original/canonical/tool), `OptionsParsed`,
  `WorkspaceStatus`. `Started.children[]` announces them all plus the
  `PatternExpandedId` (BuildBuddy reads its top-level invocation pattern
  from `Started.children[].id.pattern`, *not* from the standalone
  `Expanded` event's id).
- `CommandEnd` → `BuildFinished` (no longer terminal — `BuildMetrics`
  closes the stream now).
- `AnalysisEnd` → `TargetConfigured` + synthetic `TargetCompleted`. The
  `TargetConfigured.children[]` announces its `TargetCompleted` so BB
  flips the per-target card to `BUILT`.
- `ActionExecutionEnd` → `ActionExecuted` with `start_time` /
  `end_time` populated (BB action-timing graph; derived by subtracting
  `wall_time` from the event timestamp).
- `TestRunEnd` → `TestResult`.
- `ParsedTargetPatterns` (instant) → `PatternExpanded`. Pattern strings
  go through `normalize_root_cell_pattern` so root-cell patterns render
  as `//:foo` (Bazel-shape) instead of kuro's internal `<cell>//:foo`.
- `ConfigurationCreated` (instant) → `Configuration`.
- `make_aborted()` helper.
- `make_progress()` helper.
- `BepStreamState` accumulator: counts actions/targets across the
  stream and synthesizes a closing `BuildMetrics` event with
  `actions_executed`, `targets_configured`, `first_started_ms`,
  `last_ended_ms` (BB action-count chip + critical path).

Subscribers (`BesSubscriber`, `BepFileSubscriber`) own a
`BepStreamState`, observe each translated event, override
`handle_output` / `handle_tailer_stderr` to forward stdout/stderr as
`Progress` events, and emit the closing `BuildMetrics` in `finalize()`.

Verified against BuildBuddy (`hello_world//:main`):

| Field | Status |
|-------|--------|
| `pattern` (`["//:main"]`) | populated |
| `host` (`wg-laptop`) | populated |
| `user` (`wgray`) | populated |
| `command` (`build`) | populated |
| `actionCount` (5) | populated |
| `targetConfiguredCount` (8) | populated |
| `targetGroups[].status` (`BUILT`) | populated |
| `durationUsec` | populated |
| `invocationStatus` (`COMPLETE_INVOCATION_STATUS`) | populated |
| `success` (`true`) / `bazelExitCode` (`SUCCESS`) | populated |
| `cacheStats` | empty (out of scope until kuro records cache stats) |
| `targetGroups[].outputs` | empty (out of scope until per-target artifact tracking lands) |

Not yet covered (residual gap list):

- `WorkspaceInfo`, `Fetch`, `ConvenienceSymlinksIdentified`,
  `BuildToolLogs` — Bazel emits these but BB doesn't depend on them
  for the main invocation card.
- Per-target artifact tracking (`NamedSetOfFiles` with real outputs +
  `TargetCompleted.output_group`) — currently the `Completed` event
  is synthetic with empty outputs, which renders as "BUILT" but with
  no Outputs tab content.
- `Progress` events fire only when the daemon's tailer pushes
  stdout/stderr; for a quiet kuro build (no tailer activity) the BB
  Build Log tab stays empty.
- `--build_event_json_file` (proto3 canonical JSON) still deferred
  pending pbjson integration.

Table-driven tests: `tests/translate.rs`.

`src/translate.rs` with `BuildEventContext` + per-event-kind functions. Covered:

- `CommandStart` → `Started`
- `CommandEnd` → `BuildFinished`
- `AnalysisEnd` → `TargetConfigured`
- `ActionExecutionEnd` → `ActionExecuted` (exit code from `signed_exit_code`)
- `TestRunEnd` → `TestResult`
- `ParsedTargetPatterns` (instant) → `PatternExpanded`
- `ConfigurationCreated` (instant) → `Configuration`
- `make_aborted()` helper
- `make_progress()` helper

Not yet covered (tracked in `README.md` conformance snapshot):
`BuildMetadata`, `UnstructuredCommandLine`, `StructuredCommandLine`,
`OptionsParsed`, `WorkspaceStatus`, `Progress` accumulation,
`NamedSetOfFiles` + `TargetCompleted` with outputs, `BuildToolLogs`,
`BuildMetrics`, `ConvenienceSymlinksIdentified`.

Table-driven tests: `tests/translate.rs`.


New `app/kuro_build_event_stream/src/translate.rs`. Maps `BuckEvent` →
BEP `BuildEvent`. One visitor per Kuro event kind; lossy cases
documented inline.

Required coverage (minimum set for BuildBuddy dashboard to render
correctly):

| Kuro event | BEP event | Notes |
|---|---|---|
| `CommandStart` | `Started` | Invocation ID, command, args, start time. |
| (on build request) | `PatternExpanded` | Expanded from CLI target patterns. |
| (configuration resolution) | `Configuration` | One per platform; id = Kuro config hash. |
| `AnalysisEnd` | `TargetConfigured` | Per-target rule kind, test-or-binary flag. |
| `ActionExecutionEnd` | `ActionExecuted` | Exit code, stdout/stderr refs, mnemonic, exec kind. |
| `FinalMaterialization` | `NamedSetOfFiles` + `TargetCompleted` | Per top-level target. |
| `TestRunEnd` | `TestResult` | Per test attempt. |
| (test target summary) | `TestSummary` | Aggregated pass/fail/skipped. |
| `CommandEnd` | `BuildFinished` | Exit code, end time. |
| Cancellation path | `Aborted` | See 18.7. |
| (periodic tick) | `Progress` | stdout/stderr chunks, in-flight action count. |
| `BuildGraphExecutionInfo` | — or Kuro-specific BEP extension | Critical path is not in BEP core; emit as a custom `BuildEvent.Event.build_metadata` or a BuildBuddy-specific extension. |

Kuro-specific events with no BEP analogue (`DiceStateUpdate`,
`DiceCriticalSection`, `DynamicLambda`, `BxlExecution`) either:

- Fold into `Progress` as free-form text chunks, or
- Drop silently (document which).

Unit tests: one input event → one expected BEP output event, table-
driven.

---

### 18.3 File sinks (DONE 2026-04-24 — JSON deferred)

`src/file_sink.rs` (`FileSink` with `Binary` and `Text` encodings) +
`kuro_client_ctx/src/subscribers/bep_file_sink.rs` (`BepFileSubscriber`) wire
the existing `--build_event_binary_file` / `--build_event_text_file` flags
so they actually write BEP events (they previously accepted the flags and
dropped them).

Binary output = length-delimited prost, matching Bazel's wire format. Text
output = `Debug`-formatted, form-feed delimited (NOT proto `TextFormat` —
developer diagnostic only).

`--build_event_json_file` deferred: proto3-canonical JSON requires
`pbjson-build` integration (camelCase fields, ISO-8601 timestamps,
string-name enums). Tracked against 18.8 conformance work where byte-exact
Bazel parity matters.


Flags:
- `--build_event_json_file=<path>` — newline-delimited JSON (NDJSON),
  one `BuildEvent` per line, suitable for `jq` / log parsers.
- `--build_event_binary_file=<path>` — length-delimited proto, Bazel-
  compatible. Reads via Bazel's own tooling.
- `--build_event_text_file=<path>` — `text_format` proto, human-
  readable. Optional (Bazel has it, useful for debugging).

Implementation: each file flag wires a new `EventSink` impl
(`app/kuro_build_event_stream/src/file_sink.rs`) into the dispatcher.
Sinks are fed BEP-translated events from 18.2. Zero overhead when
flags are absent.

Ordering guarantee: BEP consumers expect in-order events keyed by
`BuildEventId` parent/child relationships. Verify that Kuro's dispatcher
preserves emission order (it should — the dispatcher is synchronous by
default per `app/kuro_events/src/dispatch.rs`).

---

### 18.4 gRPC / BES sink (DONE 2026-04-24, REOPENED + RELANDED 2026-04-24)

Initial implementation streamed events to BuildBuddy's gRPC endpoint and got
HTTP 200 back from the invocation URL — but **the invocations never showed
up on the dashboard**. HTTP 200 was misleading: BuildBuddy's web UI is an
SPA that returns 200 for any URL, so it confirmed nothing about ingestion.
Querying the public API (`SearchInvocation` / `GetInvocation`) reported
"NotFound" for every kuro invocation.

Five compounding bugs were uncovered chasing this:

1. **Always `BuildComponent::Tool`** in stream IDs. Bazel uses three
   shapes: `Controller` (no invocation_id) for `BuildEnqueued` /
   `BuildFinished`, `Controller` (with invocation_id) for the
   `InvocationAttempt*` events, `Tool` (with invocation_id) for the
   bidi tool stream. Sending everything as `Tool` left BuildBuddy with
   no controller-scope bracket to attach the tool stream to.
2. **Identical `build_id` and `invocation_id`**. Bazel mints two
   independent UUIDs (`buildRequestId` + `commandId`); BuildBuddy keys
   its routing tables off them being distinct. Now mint a fresh build_id
   in the subscriber.
3. **Bidi stream open deadlocked.**
   `client.publish_build_tool_event_stream(req_stream).await`'s
   server-side handler waits for the first request frame before
   completing setup; awaiting the open *before* feeding events meant
   we sat forever in `rx.recv()` waiting for events while the open
   blocked waiting for a request. Spawning the request feeder before
   awaiting the open call unsticks it.
4. **`InvocationAttemptFinished` / `BuildFinished` lifecycle calls
   missing `invocation_status`**. Empty status was treated as still-
   running and the invocation was never finalized. Now populated with
   `COMMAND_SUCCEEDED` + the build's exit code.
5. **`Started.options_description` empty.** The actual silent killer.
   BuildBuddy's `EventChannel.FinalizeInvocation` early-returns when
   `!hasReceivedEventWithOptions`, which is set true only when either
   `Started.options_description != ""` or an `OptionsParsed` event
   arrives. Our translator emitted `Started.options_description: ""`
   and we never sent `OptionsParsed`, so BuildBuddy *acked every
   event* and then dropped the invocation in finalization. Populated
   `options_description` with the joined sanitized argv as a stopgap;
   add a real `OptionsParsed` event in 18.2 follow-up work.

Verified post-fix: `kuro build //:main --bes_backend=grpcs://remote.buildbuddy.io ...`
produces an invocation that `GetInvocation` returns with `invocationStatus=COMPLETE_INVOCATION_STATUS`, `success=true`. The
invocation pattern (`pat=[]`) is still empty — fixing that needs
`OptionsParsed` or correct `PatternExpanded` parent/child wiring;
tracked in the 18.2 gap list.

`src/grpc_sink.rs` (`BesSink` + background uploader task) +
`kuro_client_ctx/src/subscribers/bep_bes_sink.rs` (`BesSubscriber`).

URIs: `grpcs://` / `grpc://` rewritten to `https://` / `http://` for tonic.
TLS enabled via `ClientTlsConfig::with_webpki_roots()`.

Flags wired: `--bes_backend`, `--bes_results_url`, `--bes_header`
(repeatable `KEY=VALUE`), `--bes_keywords` (CSV), `--bes_timeout`,
`--bes_upload_mode`, `--bes_instance_name`.

Lifecycle: `BuildEnqueued` → `InvocationAttemptStarted` → (bidi stream of
`bazel_event`-packed BEP events) → `ComponentStreamFinished(FINISHED)` →
`InvocationAttemptFinished` → `BuildFinished`.

Backpressure: bounded `mpsc::channel<_>` (capacity 10k); senders block on
overflow. Failure handling: connect/enqueue failures log once, mark the
sink `State::Failed`, and stop trying — BES upload never fails the user's
build.

Validated against BuildBuddy: `kuro build //:main --bes_backend=grpcs://remote.buildbuddy.io --bes_header=x-buildbuddy-api-key=<KEY>` produced an accessible invocation page at
`https://app.buildbuddy.io/invocation/<trace_id>`.


**Parity source.** `publish_build_event.proto` — `PublishLifecycleEvent`
(invocation start/attempt/finish) and `PublishBuildToolEventStream`
(per-event client streaming RPC).

Flags:
- `--bes_backend=grpc[s]://<host>:<port>` — endpoint.
- `--bes_timeout=<duration>` — how long to wait for upload before
  build tool exits.
- `--bes_upload_mode={wait_for_upload_complete,nowait,fully_async}` —
  matches Bazel.
- `--bes_header=<key>=<value>` (repeatable) — arbitrary gRPC metadata
  (BuildBuddy requires `x-buildbuddy-api-key=…`).
- `--bes_keywords=<csv>` — user-provided tags shown in BuildBuddy UI.
- `--bes_instance_name=<name>` — for BES server-side instance routing.

New sink `app/kuro_build_event_stream/src/grpc_sink.rs` using `tonic`
(already in the kuro dep tree? verify; if not, add).

Backpressure: bounded channel between dispatcher and gRPC writer.
Dropping events on overflow is not acceptable for BEP (breaks parent/
child relationships). Prefer blocking the sender; surface as a warning
if it blocks the build. Tune capacity based on Plan 17 measurements.

Failure handling:
- Transient RPC error: retry with exponential backoff, bounded by
  `--bes_timeout`.
- Terminal error: log, continue build (do not fail the user's build for
  BES-upload failure).
- Daemon shutdown with events in flight: flush before exit (respect
  `--bes_upload_mode=wait_for_upload_complete`).

---

### 18.5 Invocation metadata (PARTIAL 2026-04-24)

`BuildEventContext` (built in `streaming.rs::bep_build_event_context`) is
populated from existing invocation state:

- `invocation_id` = `trace_id`
- `build_tool_version` = `kuro_build_info::revision()`
- `workspace_directory` = `ctx.paths().project_root()`
- `working_directory` = `ctx.working_dir`
- `user` = `$USER` / `$USERNAME`
- `host` = `hostname::get()`
- `command` = subcommand name
- `cli_args` = sanitized argv

Not yet lifted into BEP `BuildMetadata` / `UnstructuredCommandLine` /
`OptionsParsed` events (see 18.2 gap list).


BEP `BuildMetadata` carries:

- `invocation_id` — map to Kuro's `trace_id` (already a UUIDv4).
- `build_id` — separate UUID per build attempt (BEP distinguishes
  invocation from attempt).
- `user` — from `$USER` / `whoami`.
- `host` — hostname.
- `workspace` — project root (Kuro already has this).
- `cmdline` — argv.
- `build_tool_version` — Kuro version string.
- `role=CI` / `role=DEV` — from `$BUILDBUDDY_ROLE` or `--bes_keywords`.

Extend `ClientContext` (`app/kuro_cli_proto/daemon.proto`) if any fields
are missing, lift at BEP `Started` emission time in the translate layer.

---

### 18.6 Results-URL surfacing (DONE 2026-04-24)

`BesSubscriber::log_results_url` emits the BuildBuddy-standard line to
stderr at subscriber construction time:

```
Streaming build results to: <prefix>/<invocation_id>
```

Currently printed once at startup; CI scrapers can find it in both success
and failure paths because it precedes any build work. Plan called for a
second emission at end-of-build; deferred until we see a concrete log
scraper that needs it.


When `--bes_results_url=<prefix>` is set, log at build start:

```
Streaming build results to: <prefix><invocation_id>
```

BuildBuddy-standard format. One-liner. Also at build end, with a
`Build completed` or `Build failed` prefix so CI log scrapers can find
it in both the success and failure paths.

---

### 18.7 Cancellation + abort path (DONE 2026-04-24)

Both `BepFileSubscriber` and `BesSubscriber` track whether they observed a
`SpanEnd::Command` event. In `finalize()` (which runs on every exit path,
including Ctrl+C via `EventsCtx::finalize_events`), if no CommandEnd was
seen they emit a synthetic `Aborted` event scoped to `BuildFinishedId` with
reason `USER_INTERRUPTED`. Closes the BES stream cleanly so BuildBuddy
transitions the invocation out of "still running" state.

Shutdown is bounded by `--bes_timeout` (default 60s); events in-flight past
that bound are dropped.


Trap SIGINT and SIGTERM before daemon teardown. Emit BEP `Aborted`
event with `reason={USER_INTERRUPTED,TIME_OUT,REMOTE_ENVIRONMENT_FAILURE,
INTERNAL}` as appropriate, then close the BES stream cleanly.

Current state: Ctrl+C during a build leaves the BuildBuddy invocation
in "still running" state indefinitely. Fixing this requires:

- Signal handler in `app/kuro_client/src/main.rs` (or wherever the
  top-level process lives).
- Coordinated shutdown: signal the dispatcher, let it emit `Aborted`,
  then let sinks flush.
- Timeout on the flush — don't hang the user's shell if BES is
  unreachable.

---

### 18.8 Conformance tests (PARTIAL 2026-04-24)

Landed: `examples/bep_diff.rs` runs a histogram diff between two BEP
binary files (typically Bazel vs Kuro). Initial snapshot captured in
`app/kuro_build_event_stream/README.md` — gap list in that table is the
working checklist for 18.2-extensions.

Not yet landed: dedicated `tests/bep_conformance/` fixture workspace (the
plan's genrule + cc_library/cc_binary + cc_test + `tags=["manual"]`
fixture) + a CI runner that re-runs the diff on every PR. `examples/hello_world`
already builds under both Kuro and Bazel and is a workable stopgap.


New directory `tests/bep_conformance/`. Small fixture workspace with:

- One `genrule` (tests action-executed coverage).
- One `cc_library` + `cc_binary` (tests target-configured / completed).
- One `cc_test` with a failing assertion (tests test-result + test-
  summary).
- One target with `tags=["manual"]` (tests filtering).

Runner script:

```sh
# Capture Bazel's BEP.
bazel build --build_event_json_file=/tmp/bazel.json //...
# Capture Kuro's BEP.
kuro build --build_event_json_file=/tmp/kuro.json //...
# Diff (ignoring volatile fields).
python3 tools/bep_diff.py /tmp/bazel.json /tmp/kuro.json
```

`bep_diff.py` normalizes UUIDs, timestamps, workspace paths, absolute
artifact paths before comparison. Permitted differences catalogued in
the test readme; any unpermitted difference fails CI.

---

### 18.10 BB Timing tab + Action-digest determinism (DONE 2026-04-29)

This phase covers the cluster of follow-ups required to make the
BuildBuddy invocation page actually render every panel kuro had
half-populated, and to make kuro's RE action digests stable enough
that BuildBuddy's CAS hits for the same logical action across daemon
restarts. The work split into three independent fixes plus a sweep of
proto/tracing wiring; documenting them together because they were all
discovered (and verified) by running `kuro build @llvm-project//llvm`
end-to-end against BuildBuddy and watching the dashboard.

#### 18.10.1 BB Timing tab parity

The `command.profile.gz` chrome-trace upload (18.4) lit up the Timing
tab's *flamegraph*, but every other Timing-tab card was empty:

| BB card | Driver | Status before | Fix |
|---|---|---|---|
| Flamegraph timeline | `command.profile.gz` `X` events | Stuck on "Build is in progress…" for clang-scale | Captured `local_thread_id` + tokio thread name on `ActionExecutionEnd`, used as chrome-trace `tid`; lane labels read off `ThreadName` so BB shows `kuro-rt-N` instead of `Worker N`. Required `data.proto` fields `local_thread_id`/`local_thread_name` (47/48), per-worker tokio name via `thread_name_fn`, monotonic per-thread index in `kuro_util::threads::thread_index()` (mirrors `java.lang.Thread.getId()`). |
| Phase Breakdown pie (Launch / Evaluation / Analysis / Execution) | `traceEvents` whose `name` is `buildTargets` / `runAnalysisPhase` / `evaluateTargetPatterns` / `Launch Blaze` | Card invisible (filter array filters to empty) | Synthetic `buildTargets` X-event covering `[command_start_us, command_end_us]` so BB sees ≥1 phase slice; remaining phase markers deferred until kuro plumbs per-phase timestamps to the translate layer. |
| Execution Breakdown pie (Executing locally / Executing remotely / Checking cache hits / …) | trace events keyed by name `subprocess.run`, `execute remotely`, `check cache hit` (or `cat` `local action execution`, `remote output download`) | Card invisible | Per-action *companion* trace event with `cat: "general information"` and `name` selected by `ActionExecutionKind`: `subprocess.run` (Local/LocalWorker), `execute remotely` (Remote), `check cache hit` (ActionCache / LocalDepFile / RemoteDepFileCache). The descriptive event (`cat: "action processing"`, name = `<mnemonic> <label>`) stays so the timeline tooltip is still useful — BB's breakdown only sums by name. |
| BuildMetrics chips (Action Count, Local Action Cache Hits, runner_count split, target_metrics, timing_metrics, build_graph_metrics) | `BuildMetrics` event from BES stream | Action count populated; everything else empty | Extended `BepStreamState` with per-mnemonic `MnemonicStats` map (actions_executed / first_started_ms / last_ended_ms), per-executor-kind `runner_counts`, ActionCache hit/miss split, `total_action_wall_us`, plus kuro-side accumulators populated by a new `observe_kuro_event(&BuckEvent)` method (Command-span timestamps, BuildGraphInfo critical-path stats, periodic Snapshot samples). `build_metrics_event()` now emits a Bazel-shape `BuildMetrics` with `action_summary.{actions_created, actions_executed, action_data[], runner_count[], action_cache_statistics}`, `target_metrics`, `timing_metrics.{cpu_time_in_ms, wall_time_in_ms, execution_phase_time_in_ms, critical_path_time}`, `build_graph_metrics`. |
| Time-series line plots panel (CPU usage, Memory usage, System load, Network up/down) — between flamegraph and Breakdown | chrome-trace `C` (counter) events with names matching BB's `TIME_SERIES_METADATA` (`CPU usage (Bazel)`, `Memory usage (Bazel)`, `System load average`, `Network Up/Down usage (total)`, etc.) | Panel hidden (filtered out when no series) | Subscribe to `Instant::Snapshot` events in `BepStreamState`; sample `kuro_rss_bytes`, `kuro_user_cpu_us+kuro_system_cpu_us`, `host_cpu_usage_*`, `unix_system_stats.load1`, sum of `network_interface_stats.{tx,rx}_bytes`. In `build_profile_json()` emit one counter event per series per snapshot — level series (memory, load) directly, rate series (CPU cores = Δus/Δus, network Mbps = Δbytes·8/1e6/Δs) differenced against the prior snapshot. |

Other BES-side fixes that landed in the same arc:

- **Streaming-RPC lifetime**: removed `Endpoint::timeout(config.timeout)`
  from `grpc_sink::build_endpoint`. tonic was applying it as a *per-RPC
  deadline*, which cancelled `PublishBuildToolEventStream` mid-build at
  the default 60 s — BB received the events that fit in the first
  minute and the trailing `BuildToolLogs` / `BuildMetrics` / lifecycle
  events were silently dropped. The deadline now only gates *shutdown*
  via `tokio::time::timeout(timeout, upload_task)` in `BesSink::shutdown`.
- **`build_id` cross-correlation**: chrome-trace `otherData.build_id`
  populated from `BesConfig.invocation_id` (was hardcoded `"kuro"`).
  Required because BB cross-checks the trace's `build_id` against the
  BES stream's invocation id; mismatch ⇒ BB drops the trace and the
  Timing tab stays at "Build is in progress…" even when the upload
  completed.
- **`bazel_version` shape**: `release 8.0.0-kuro` (was bare `kuro`).
  BB's parser bails on an absent `release ` token at clang scale.
- **Trace `tid` semantics**: per-thread monotonic counter, not OS tid.
  Bazel uses `java.lang.Thread.getId()` which is a monotonic counter
  too; `gettid(2)` would have been observably-different across daemon
  restarts and confused BB's lane stitching.
- **chrono on dates**: `BepStreamState` writes `otherData.date` as
  `chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.9fZ")` instead of the
  hand-rolled civil-date math.

Verified end-to-end on
`kuro build @llvm-project//clang:clang --config=remote`: Timing tab
flamegraph populated with named `kuro-rt-N` lanes, Phase Breakdown +
Execution Breakdown both render, BuildMetrics chips populated, and
the time-series line plots show CPU/memory/network/load over the
build's wall window.

Files: `app/kuro_build_event_stream/src/translate.rs` (the bulk of the
state-keeping + JSON emit), `app/kuro_build_event_stream/src/grpc_sink.rs`
(timeout fix), `app/kuro_client_ctx/src/subscribers/bep_bes_sink.rs` +
`bep_file_sink.rs` (`observe_kuro_event` wiring), `app/kuro_data/data.proto`
(thread fields), `app/kuro_build_api/src/actions/calculation.rs`
(thread capture site), `app/kuro_util/src/{threads.rs,tokio_runtime.rs}`
(thread_index + per-worker tokio name).

#### 18.10.2 RE action-digest determinism (canonical-name fix)

End-to-end test: build `@llvm-project//llvm:Support` in a daemon,
`kuro killall && rm -rf buck-out/v2/cache buck-out/v2/forkserver`,
build again. Expected: ≈100% BB action-cache hits on the second build.
Observed (pre-fix): 22% hits — even though every per-action input root
matches and every command/env hash matches, the action digest itself
differs across the two builds.

**Root cause** — *two* code paths in kuro disagreed on the canonical
prefix for repos generated by an extension defined in the **root**
module:

- `pending_repo_cells.rs` (Bazel-correct): root module's own extension
  → canonical name `_main+<ext>+<repo>`. This is what Bazel writes
  too, and what `pending_repo_cells.rs` registers as the cell path
  `bazel-external/_main+<ext>+<repo>`.
- `extension_execution_dice.rs::extract_owning_module` (and the
  callers that reused it — `extension_repo.rs:481`,
  `module_extension_executor_impl.rs:438`): returned the root module's
  *declared* name from `module(name=…)`, e.g. `llvm-project-overlay`,
  yielding `llvm-project-overlay+<ext>+<repo>`.

Result: kuro materialized one path under bazel-external (the `_main+…`
one) while later code paths registered dynamic cells / probed file
paths under the *other* spelling, which never existed. Symptom #1 was
"package `llvm-project//llvm` does not exist" after `kuro clean`
(because the extension repo's own files were looked up under the
non-materialized name); Symptom #2 was that the wrong canonical leaked
into compile commands as `-I bazel-external/<wrong-prefix>/...`,
making *every* compile action's digest unique to the kuro daemon
that produced it. BuildBuddy correctly stored the result under each
unique digest but the next daemon couldn't find any of them.

**Fix.** `extract_owning_module(extension_id, root_module_name)` now
substitutes `_main` when the extension's owning module matches the
root module's declared name. `build_canonical_names` and
`ModuleExtensionResult::new` thread `root_module_name` through.
`extension_repo.rs` reads canonical names off `ext_result.canonical_names`
(single source of truth from `build_canonical_names`) instead of
recomputing them without root-module context.
`module_extension_executor_impl.rs` already had `root_module_name` in
scope at the call site; just passes it now.

Verification: `@llvm-project//llvm:Support` daemon-restart test went
from 0/183 cache hits to 183/183 cache hits — and *the kuro digests
matched bazel's*, because the `_main+…` prefix is what bazel's RE
client stores too.

Files: `app/kuro_bzlmod/src/extension_execution_dice.rs`,
`app/kuro_bzlmod/src/pending_repo_cells.rs` (test fixtures),
`app/kuro_external_cells/src/extension_repo.rs`,
`app/kuro_interpreter_for_build/src/module_extension_executor_impl.rs`.

#### 18.10.3 Compile-flag determinism (`EXTERNAL_INCLUDE_DIRS` ordering)

After 18.10.2 small builds hit 100%, but the full
`@llvm-project//llvm:llvm` (4.8 k actions) still got only 23% hits
across kuro→kuro restarts. Per-field hashing of every
`RE::Command` field — added under `tracing::debug!(target =
"action_digest_debug", …)` in `command_executor::prepare_action` —
narrowed it to `args_hash` differing for ~3700 c_compile actions.
Dumping the full args list for one specific action (`AdornedCFG.pic.o`)
across two runs showed the *same set* of `-idirafter` flags but in
*different order*.

**Root cause.** `app/kuro_build_api/src/interpreter/rule_defs/cc_common/mod.rs`
defines `EXTERNAL_INCLUDE_DIRS: Mutex<Vec<String>>`, a process-global
mutable registry populated by `register_external_include_dir(...)`
from inside `cc_common.compile()` and the native `cc_library` stub.
Order of insertion is whatever the parallel analysis scheduler picks —
non-deterministic across daemon restarts. `get_external_include_dirs()`
returned the vec verbatim, so action prep emitted `-idirafter` flags
in that random order. Compile-command bytes differed → command_digest
differed → action_digest differed → BB cache miss.

**Fix.** `get_external_include_dirs()` now sorts the vec
lexicographically before returning. Sort is safe for `-idirafter`
flags (lowest-priority include search; cross-repo dirs don't shadow
each other in the LLVM/clang case verified, and Bazel itself doesn't
guarantee a specific order between them).

Verified: kuro→kuro warm went from 23% hits / 232 s wall to 75% hits /
81 s wall (2.9× faster on the warm path).

The remaining 25% miss rate is a *deeper* architectural issue that
this fix only attenuates: `EXTERNAL_INCLUDE_DIRS` membership itself
varies across runs, because `register_external_include_dir` is called
during action prep and *which* actions have been prepped by the time
a particular target's prep runs depends on scheduler order. So an
action prepped early sees fewer registered dirs than one prepped late.
The proper fix is to retire the global mutable registry and gather
include dirs from `cc_compilation_context` deps directly at action
prep time (proper DICE pure-function model) — see follow-up below.

Files: `app/kuro_build_api/src/interpreter/rule_defs/cc_common/mod.rs`
(sort + FIXME comment naming the architectural issue),
`app/kuro_execute/src/execute/command_executor.rs` (per-field
diagnostic tracing under `action_digest_debug` target),
`app/kuro_execute_impl/src/executors/action_cache.rs` (HIT/MISS/ERR
outcome tracing under `action_cache_query` target).

#### 18.10 follow-ups (open)

- **Retire `EXTERNAL_INCLUDE_DIRS`** in favour of dep-traversal
  gathering at action prep time. Closes the remaining 25% miss-rate
  gap on the warm path. Tracking this as 18.10.4 once a real consumer
  rule lands; the sort fix above is good enough for the current
  benchmark numbers.
- **Phase markers in chrome trace.** Today only the synthetic
  `buildTargets` event covers the full build span; emit
  `evaluateTargetPatterns` / `runAnalysisPhase` / `Launch Blaze`
  when kuro grows per-phase timestamps in `BepStreamState`.
- **Memory / package / artifact metrics in `BuildMetrics`.** The
  Bazel proto exposes them; kuro doesn't yet record the underlying
  data on the BES side. Defer until a user asks.

---

### 18.9 OTLP traces (OPEN, P2)

**Parity source.** OpenTelemetry Protocol, `opentelemetry-otlp` crate.

Separate from BEP. A subset of users want Jaeger / Tempo / Grafana
traces alongside BEP. Flag: `--trace-endpoint=otlp+grpc://...`.

New sink `app/kuro_build_event_stream/src/otlp_sink.rs`. Maps Kuro
span-start/span-end pairs to OTLP spans, with trace_id = Kuro trace_id.

Defer unless a user explicitly asks — BEP covers most of the same
observability surface and BuildBuddy doesn't require OTLP.

---

## Dependencies and ordering

```
Plan 16 + 17 complete
  │
  ▼
18.1 (vendor protos) ──► 18.2 (translate layer)
                         │
                         ├─► 18.3 (file sinks) ────► 18.8 (conformance)
                         │
                         └─► 18.4 (gRPC sink) ─────►
                                                    │
                                                    ├─► 18.5 (metadata)
                                                    ├─► 18.6 (results URL)
                                                    └─► 18.7 (cancellation)

18.9 (OTLP) independent — standalone.
```

Recommended: **18.1 → 18.2 → 18.3 → 18.8 (file-based conformance)
→ 18.4 → 18.5 → 18.6 → 18.7 → (18.9 if asked)**.

The file-sink path (18.3) before the gRPC path (18.4) means we can
validate the translation layer against Bazel's own BEP output offline
before touching the network.

## Open questions

- How much of BuildBuddy's "Invocation diff" feature relies on fields
  Kuro doesn't carry? (e.g., per-action input digests, remote-cache
  instance-name metadata). Answer during 18.8 by diffing a real
  BuildBuddy invocation for identical workspace.
- Should Kuro emit a BuildBuddy-specific BEP extension for critical
  path, or drop it onto the floor for BEP consumers? Lean toward
  emitting as a `BuildMetadata` key-value for broader consumer
  compatibility.
- Do we need backwards compat with Bazel 7/8 BEP? Current call: no —
  target 9.x only, matches the rest of the plan-15 scope.

## Success criteria

- `kuro build --build_event_json_file=… //...` on the conformance
  fixture produces BEP JSON that diffs clean against Bazel's output
  (modulo documented volatile fields).
- `kuro build --bes_backend=grpcs://app.buildbuddy.io …
  @llvm-project//clang:clang` produces a BuildBuddy invocation page
  with: target list, critical path visible, action-level timings,
  cache stats, build-log text tab populated.
- Ctrl+C during a build leaves the invocation in `cancelled` state in
  BuildBuddy (not `running`).
- BEP-related flags impose <1% overhead when enabled, 0% when unset
  (measured via Plan 16 harness).
- Conformance test green in CI for every supported BEP event type.
