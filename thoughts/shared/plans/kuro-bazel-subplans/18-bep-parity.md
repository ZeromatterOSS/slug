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

### 18.1 Vendor BEP proto schema (OPEN)

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

### 18.2 Translation layer (OPEN)

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

### 18.3 File sinks (OPEN)

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

### 18.4 gRPC / BES sink (OPEN)

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

### 18.5 Invocation metadata (OPEN)

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

### 18.6 Results-URL surfacing (OPEN)

When `--bes_results_url=<prefix>` is set, log at build start:

```
Streaming build results to: <prefix><invocation_id>
```

BuildBuddy-standard format. One-liner. Also at build end, with a
`Build completed` or `Build failed` prefix so CI log scrapers can find
it in both the success and failure paths.

---

### 18.7 Cancellation + abort path (OPEN)

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

### 18.8 Conformance tests (OPEN)

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
