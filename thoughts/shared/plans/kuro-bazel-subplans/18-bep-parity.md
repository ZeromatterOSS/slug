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
