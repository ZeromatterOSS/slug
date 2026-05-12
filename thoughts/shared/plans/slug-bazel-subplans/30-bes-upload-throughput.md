# Plan 30: BES upload throughput

> Successor follow-up to the 2026-04-29 BES profiling pass that
> established the wall-time gap between `slug` and `bazel` on a fully-
> warm `@llvm-project//llvm` build is dominated by the post-build BES
> upload wait — slug builds in 22.4 s, then blocks for 23 s draining
> trailing events + lifecycle ACKs before exiting. With
> `--bes_upload_mode=nowait` the wall drops to 24.7 s (vs bazel's 50.7
> s); the goal of this plan is to close the gap *without* requiring
> users to opt out of upload completion.

## Goal

Match Bazel's BES upload behaviour for the default
`wait_for_upload_complete` mode: ≥99% of BEP events should already be
ACKed by the server *at the moment the build phase ends*, so the
post-build `BesSink::shutdown()` wait is sub-second. Stretch goal:
slug wall ≤ bazel wall on warm `@llvm-project//llvm`.

## Background

### What we measured

Profiling pass on `slug→slug warm @llvm-project//llvm --config=remote`
(commit `9af3642e`):

| Phase | Time |
|---|---|
| `bzlmod` + analysis + execute + materialize (CommandStart→CommandEnd) | 22.4 s |
| `command.profile.gz` chrome trace ByteStream upload | 0.3–3.2 s |
| `BesSink::shutdown()` — feeder drain + drain_task ACKs + lifecycle close | **23.2 s** |
| Subscriber finalize (others) | <1 s |
| Wall (default `wait_for_upload_complete`) | **57.4 s** |
| Wall (`--bes_upload_mode=nowait`) | **24.7 s** |

The 23.2 s post-build wait is the entirety of the gap to `bazel`'s 50.7 s.

### Where the wait comes from

`run_uploader` in `app/slug_build_event_stream/src/grpc_sink.rs` runs
two tasks for one BES bidi stream:

1. `feeder_task`: pulls events from the per-build mpsc, wraps each as
   `PublishBuildToolEventStreamRequest{ ordered_build_event: …,
   sequence_number: seq++ }`, forwards into a `ReceiverStream` that
   tonic feeds onto the wire.
2. `drain_task`: reads ACK responses from the response stream until
   the server half-closes.

`BesSink::shutdown()` (called from `BesSubscriber::finalize()`) sends
the `Finish` marker, then awaits the `upload_task` JoinHandle. The
upload_task returns only after:
- feeder drains all queued events,
- drain_task observes the final response (server-side stream close),
- two trailing `publish_lifecycle` unary calls (`InvocationAttemptFinished`
  + `BuildFinished`) complete.

For 4853 BEP events at build end ≈ 350 events still in flight (event
production rate during the 22.4 s build ≈ 220 events/s; gRPC stream
throughput observed ≈ 200–220 events/s on the BB.io public endpoint).
At ~65 ms server-side ACK latency, draining 350 events takes ~23 s.
That math matches what we measured.

### Why bazel doesn't pay this cost

Single-stream-per-build with serial server-side ACKs is the *same*
constraint Bazel hits — bazel can't do better than the BB server's
processing rate either. The difference is *when* the queue empties:

- bazel's `BuildEventServiceUploader` (`src/main/java/com/google/devtools/build/lib/buildeventservice/BuildEventServiceUploader.java`)
  is started early, during command setup — it has an open stream
  before the first action runs.
- During the build, bazel's queue drains continuously. By the time the
  build phase ends, only the *literal* last events (BuildFinished,
  BuildToolLogs, BuildMetrics) are still in flight, so the close is
  fast.
- bazel additionally sends events fire-and-forget at the application
  layer: `streamContext.sendOverStream(bazelEvent)` (line 478 of
  `BuildEventServiceUploader.java`) does *not* wait for the previous
  ACK before sending the next event. Acks arrive asynchronously via a
  `StreamObserver` callback that just enqueues `Command.AckReceived`
  back into the same command queue.

Slug's pipeline is also fire-and-forget (the mpsc → ReceiverStream
hop is non-blocking when capacity exists), so the in-flight depth at
build end is the lever — and *that* is set by **how early the stream
opens** and **how fast events flow into it during the build**.

### What buck2 / bonanza tell us

- **buck2** has no BES uploader (it uses Meta-internal Scribe
  telemetry). It is *not* a reference for BES upload performance.
  However, `app/buck2_oss_re_grpc/src/client.rs` shows two
  applicable patterns:
  - Per-service tonic Channel (CAS / Execute / ActionCache / ByteStream
    / Capabilities each get their own `Channel::builder(uri).connect()`
    — 5 channels per backend). This protects high-priority RPCs from
    being head-of-line-blocked by ByteStream uploads on the same
    HTTP/2 connection.
  - `buffer_unordered(concurrency_limit)` fan-out for blob upload
    parallelism. Doesn't directly apply to BES (which is one logical
    stream per build) but the pattern is useful for the trailing
    `command.profile.gz` ByteStream upload.
- **bonanza** is the buildbarn server-side reimplementation of bazel
  (`pkg/scheduler/in_memory_build_queue.go`, `cmd/bonanza_*`); no
  client-side BES uploader code. Not directly relevant.
- **bazel** is the canonical reference; pattern documented above.

### Library / protocol substrate assessment

- **tonic 0.12.3 + prost 0.13.4** (current; same versions buck2 uses).
  Mature, hyper-based, optimized for HTTP/2 streaming. No obvious
  newer-major version with breaking-perf wins.
- **bufbuild/connect-rs (Connect protocol)**: Buf's "Connect" client
  in Rust. **Archived 2024-06**, no longer maintained. Even if it
  weren't, BB's BES endpoint speaks gRPC-on-HTTP/2 which connect-rs
  primarily serves over plain HTTP, so we'd lose bidirectional
  streaming semantics. *Discard.*
- **anthropics/connect-rust 0.3 + anthropics/buffa 0.3/0.4**
  (`connectrpc` + `buffa` crates): the *actively developed* successor
  to the bufbuild project, open-sourced March 2026 by Anthropic. The
  pair is the structural analog of tonic+prost — connect-rust is the
  transport layer (Tower service, hyper+h2, supports Connect, gRPC,
  and gRPC-Web on both client and server, passes all 6,558 ConnectRPC
  conformance tests; bidi over native gRPC-on-HTTP/2 works), and
  buffa is the message codegen + runtime (owned `M` + zero-copy
  `MView<'a>` view types, binary/JSON/text codecs). Public benchmark
  shows 33% higher throughput than tonic+prost and 3.6% vs 9.6%
  allocator CPU on a decode-heavy 22 KB-batch workload, attributed
  to buffa's zero-copy view path avoiding per-string allocation on
  decode. **However, not viable for the BES path today**, for four
  stacked reasons:
  1. **No prost mode in connect-rust.** It generates clients against
     buffa view types only. There is no compatibility shim with
     prost-generated `PublishBuildToolEventStreamRequest` / BEP
     message types. Migrating BES would force a full buffa
     regeneration of the BES + BEP + RE API schemas (hundreds of
     messages), with no gradual interop period — `Option<Box<M>>`
     becomes `MessageField<M>`, enums change from `i32` to
     `EnumValue<T>`, well-known types lose primitive mappings,
     encode/decode signatures differ. Anthropic's own
     [prost migration guide](https://github.com/anthropics/buffa/blob/main/docs/migration-from-prost.md)
     classifies the effort as "High."
  2. **No HTTP/2 flow-control tunables exposed.** The h2 stack
     underneath is the same crate tonic uses, but connect-rust does
     not surface `initial_stream_window_size`,
     `initial_connection_window_size`, `http2_keep_alive_interval`,
     or `tcp_nodelay`. Tonic exposes all of these on
     `Channel::from_shared(...)`. Since 30.2's whole premise is
     turning those knobs, switching transport would *remove* the
     lever we want to pull.
  3. **Workload mismatch.** buffa's win is on decode-heavy paths
     with large strings/bytes/nested fields. The BES uploader is the
     opposite — small (1–2 KB) outbound messages, encode-only on the
     client, bottlenecked on server-side per-event ACK latency on a
     single bidi stream. The same hyper+h2 transport sits underneath
     both libraries, so the wire-level streaming behaviour is
     identical; the 33% number does not transfer to this workload.
  4. **Pre-1.0, breaking churn.** connect-rust shipped 0.2.0 →
     0.3.3 between 2026-03-17 and 2026-04-17, and the changelog
     already advertises a breaking 0.4 with `ConnectError` /
     handler-signature changes. buffa is at 0.4.0 (2026-04-27). Not
     appropriate for a production RPC path that already works.

  *Discard for BES specifically.* Worth re-evaluating if a future
  slug path is decode-heavy on the client (e.g., bulk
  ActionResult fan-in from RE) — buffa's view types could plausibly
  matter there. Out of scope for plan 30.
- **grpcio (grpc-rs)**: Wraps C++ grpc-core. Higher per-call
  throughput in microbenchmarks, but adds a C++ dependency, has
  weaker async ergonomics, and Meta has been migrating away from it
  inside buck2. *Discard.*
- **prost alternatives** (bilrost, quick-protobuf): unrelated to gRPC
  performance — they only change wire encoding speed for individual
  messages, which is ~1% of our BES wall budget. *Out of scope.*

So the substrate is fine. The win is in **how we *use* tonic**, not
which library. The candidate replacement (connect-rust 0.3 + buffa
0.3/0.4) is real and actively developed — it just doesn't help
*this* bottleneck: it sits on the same hyper+h2 transport, hides the
flow-control knobs we want to tune, and would require a full
schema-wide buffa migration before a single byte moved differently
on the wire. Both gRPC clients hit the same single-stream BES
protocol with the same per-event server ACK latency.

## Scope

In:

- Earlier BES stream open: connect on CLI start, not on first event.
- Daemon-resident BES uploader: keep `run_uploader` running across
  CLI process exit so the client doesn't block on it.
- Tonic flow-control tunables on the BES Channel (initial stream
  window, http2 keepalive, etc.) — match what buck2's re_grpc uses.
- Per-service tonic Channel (BES on its own connection, not sharing
  with ByteStream).
- Upgrade `--bes_upload_mode=fully_async` so it actually means "keep
  uploading after slug CLI exits" (today it's just an alias for
  `nowait`).

Out:

- Replacing tonic / prost. Not a viable opportunity (above).
- Multi-stream BES (parallel `PublishBuildToolEventStream` calls).
  The protocol forbids this for a single (build_id, component)
  pair — sequence numbers must be monotonic per stream. Bazel doesn't
  do it either.
- Server-side optimization (BB.io's per-event ACK latency). Out of
  our control.

## Current State Analysis

### `BesSink::start()` is lazy

`bep_bes_sink.rs::ensure_connected` defers the call to
`BesSink::start` until the first `handle_events`/`handle_output`/
`handle_tailer_stderr`. `BesSink::start` then:

1. `build_endpoint(...)`: creates `tonic::transport::Endpoint`
   (~10 µs).
2. `endpoint.connect()`: TCP handshake → TLS handshake →
   HTTP/2 SETTINGS exchange. **~150–400 ms on a typical network.**
3. spawns `run_uploader`.
4. `run_uploader` sends `BuildEnqueued` + `InvocationAttemptStarted`
   lifecycle events as **two unary RPCs** (~2 × 100 ms RTTs) before
   opening the bidi stream.
5. The bidi stream then opens (one more RTT).

Total cold-start: **~500–800 ms** of stream-warmup. Bazel's uploader
is started earlier and runs all this in parallel with command setup,
so the stream is open by the time the first action event lands.

### `run_uploader` shutdown is sequential

After feeder + drain finish, `run_uploader` makes two more **unary
RPCs** (`InvocationAttemptFinished`, `BuildFinished`) sequentially.
Each is one RTT (~50–100 ms). Together: ~100–200 ms tacked onto the
post-build wait. Not the dominant cost, but accumulates.

### tonic Channel is configured with only `connect_timeout(10s)`

`grpc_sink.rs::build_endpoint` sets only `connect_timeout` and
`tls_config`. No `http2_keep_alive_interval`, no
`initial_stream_window_size` override, no `tcp_nodelay`. Defaults are
fine for steady-state but conservative for burst sends:

| Tonic default | Could be |
|---|---|
| `initial_stream_window_size`: 65 KiB | 1–2 MiB |
| `initial_connection_window_size`: 64 KiB | 4–8 MiB |
| HTTP/2 keepalive: off | 30 s ping, 20 s timeout (matches buck2) |
| TCP nodelay: tonic default (depends on connector) | explicit on |

For a single bidi stream pumping ~4800 events of ~1–2 KB each, a 64 KB
initial window means the stream stalls every ~30–60 events waiting for
WINDOW_UPDATE. Bumping it removes a class of micro-stalls that
compounds over the 22 s build phase.

### BES Channel is a single shared `tonic::transport::Channel`

`BesSink` stores one `channel`. ByteStream upload of `command.profile.gz`
*also* uses this channel (`BesSink::upload_blob_bytestream`). Since
ByteStream is sent before BES finalizes BuildMetrics, large-trace
uploads can starve the BES bidi stream's flow control budget on the
shared connection. Buck2's re_grpc keeps ByteStream and Execute on
separate Channels for exactly this reason.

## Phases

### 30.1 Earlier stream open (single-task win) — DONE 2026-04-29

Open the BES connection + lifecycle handshake at `BesSubscriber::new`
(equivalently: at the moment we know `--bes_backend` is set), not on
first event. This shifts ~500–800 ms of stream-warmup out of the
critical path of the *first* action.

Mechanism: change `BesSubscriber::maybe_new` to spawn a
`tokio::task` that calls `BesSink::start(config).await` in the
background and stashes the resulting `Arc<BesSink>` in
`State::Pending` → `State::Connecting(JoinHandle)` → `State::Connected`.
`ensure_connected` awaits the JoinHandle if still running.

Risk: minimal. If config is bad (bad URL, bad cert), the failure mode
is the same as today — connect error, log warning, BES disabled for
this build.

Test: instrument `BesSink::start` start/end timestamps, run hello_world
build, verify stream-open completes before first action's
`handle_events`.

### 30.2 Tonic flow-control + per-service Channel — DONE 2026-04-29

Mirror buck2's re_grpc tunables on the BES `Channel`:

```rust
let endpoint = endpoint
    .connect_timeout(Duration::from_secs(10))
    .http2_keep_alive_interval(Duration::from_secs(30))
    .keep_alive_timeout(Duration::from_secs(20))
    .keep_alive_while_idle(true)
    .initial_stream_window_size(2 * 1024 * 1024)
    .initial_connection_window_size(8 * 1024 * 1024)
    .tcp_nodelay(true);
```

Split BES Channel from ByteStream Channel: today
`upload_blob_bytestream` borrows the same `BesSink::channel`. Give
the chrome-trace upload its own short-lived Channel. The BES bidi
stream then doesn't share its connection-level flow-control budget
with a multi-MB blob upload.

Risk: minor. Connect window changes are server-tolerated. Channel
duplication adds one extra TCP+TLS handshake (~150 ms) but happens
during finalize where it overlaps with the BES drain anyway.

Test: re-run the full `@llvm-project//llvm` warm benchmark; expect
1–2 s improvement in trace upload step + smoother streaming during
build.

### 30.3 Daemon-resident BES uploader (the big win) — DEFERRED to plan 31.3

> Plan 31 (`31-bazel-perf-parity.md`) takes ownership of the actual
> landing of this phase. The scope here is preserved for archaeology;
> see `31-bazel-perf-parity.md` §31.3 for the file-level changes.

Move `BesSink` from the client process to the daemon process. The
client emits BEP events to the daemon (already does, via the BuckEvent
stream); the daemon's BES uploader keeps running across client
invocations.

Implementation sketch:

- BesSubscriber moves from `slug_client_ctx` to
  `slug_server_commands` (or similar daemon-side crate).
- The daemon owns a long-lived BesSink keyed by invocation_id.
  Created on `CommandStart`, dies on `CommandEnd + drain complete`,
  but its drain runs *out of band* with the client connection.
- The client process exits as soon as BuildFinished is emitted; it
  does *not* wait for BesSink::shutdown.
- Daemon's BES uploader runs to completion on its own — no client
  involvement.
- For `--bes_upload_mode=wait_for_upload_complete` (default), the
  client waits *only* if the user explicitly asked for it via a flag
  like `--bes_upload_block_client=true`. Otherwise daemon-async is
  the default.

This is the most expensive change but has the highest ROI: client
wall = build wall; BES upload runs entirely in the background.

Risk: medium-high. Architectural change. The daemon needs to manage
multiple concurrent BES sinks (if user runs back-to-back builds),
and drain them on daemon shutdown so they're not lost. Also: server
log already shows daemon-side processing of BEP events (via
`slug_data` events that translate to BEP); careful work to keep the
two paths consistent.

Test: same warm benchmark; measure both *client* wall and daemon-side
upload completion time. Client wall should match the `nowait` case
(~25 s) while still guaranteeing upload completion before next
invocation starts.

### 30.4 (DONE 2026-04-29) Honor `--bes_upload_mode=nowait`

Already landed in commit `9af3642e`. `BesSink::shutdown` now respects
`config.upload_mode`:

- `WaitForUploadComplete` (default): unchanged behaviour.
- `NoWait` / `FullyAsync`: drop the `JoinHandle` without awaiting.

Verification: `slug build … --bes_upload_mode=nowait` reduces
`@llvm-project//llvm` warm wall from 57 s → 24.7 s on the same
benchmark.

`FullyAsync` is a degenerate alias for `NoWait` until 30.3 lands —
without a daemon-resident uploader the client process exits anyway
and the spawned task is killed. Plan 30.3 makes `FullyAsync` honor
its name.

### 30.5 Lifecycle handshake parallelization — DONE 2026-04-29

`run_uploader` opens the bidi stream only after `BuildEnqueued` and
`InvocationAttemptStarted` complete. Make those concurrent:
`tokio::join!(send_enqueue, send_attempt_started)`. They're independent
and BB ingests them in any order. Saves 1–2 RTTs (~100–200 ms).

Symmetric on the close side: parallelize `InvocationAttemptFinished`
and `BuildFinished`. Saves another ~100 ms on shutdown.

Risk: trivial. Each lifecycle is independent and idempotent.

Test: trace timestamps of each lifecycle event; verify they overlap.

## Dependencies and ordering

```text
30.1 (earlier open) ──┐
                      ├─► 30.5 (parallel lifecycle) ──┐
30.2 (tonic tuning) ──┘                               │
                                                      ▼
                                     30.3 (daemon-resident uploader)
30.4 (DONE) ──────────────────────────────────────────┘
```

Recommended landing order: 30.1 + 30.2 + 30.5 (small, independent
wins) → 30.3 (architectural). 30.4 already landed.

After 30.1 + 30.2 + 30.5 alone, expect ~5–10 s reduction (cold-start +
flow-control + lifecycle overhead). After 30.3, the post-build wait
disappears entirely from client wall, putting slug at ~25 s vs bazel's
50 s.

## Open questions

- **Multi-invocation BES on a single daemon**: if user runs `slug
  build A` then `slug build B` while A's BES upload is still in
  flight, do we run them on parallel TCP connections, or queue B's
  upload behind A on the same Channel? Bazel doesn't have this
  problem (process-per-invocation). Probably parallel connections —
  matches buck2's per-service Channel pattern.
- **Daemon shutdown durability**: if the daemon is killed mid-upload,
  should we have a retry queue or just drop? Bazel drops; matches
  the documented `wait_for_upload_complete` semantics ("client
  process exit is the durability boundary"). Daemon-resident moves
  the boundary to daemon process exit; mostly a non-issue since the
  daemon is long-lived.
- **`--bes_results_url` semantics under daemon-async**: the URL is
  printed at build start; users click it to see live progress on
  BB. With daemon-async, the upload may not have *started* by the
  time the user clicks. Currently slug sends `BuildEnqueued` early,
  so BB has the invocation ID immediately. Should still work but
  worth verifying in 30.3.

## Success criteria

- `slug build @llvm-project//llvm --config=remote` (warm): client
  wall ≤ 30 s on the same hardware/network where it's 57 s today.
- Default `--bes_upload_mode=wait_for_upload_complete` produces
  identical BB.io invocation page contents (BuildMetrics, Timing
  tab, action list) compared to today.
- `slug build … && slug build …` back-to-back: second invocation
  starts immediately, and both finalize their BES upload (verified
  via BB.io) within ~5 s of their respective build phases ending.
- No measurable correctness regression on `hello_world`,
  `@llvm-project//clang:basic`, or `@llvm-project//llvm` builds.

## What this plan is NOT

- Not a tonic vs grpcio comparison: the substrate isn't the
  bottleneck.
- Not a connect-rust / buffa migration. The newer
  anthropics/connect-rust 0.3 + buffa 0.3 stack *does* speak gRPC-on-
  HTTP/2 with bidi (so the old "wrong protocol" objection no longer
  applies), but it (a) is hard-coupled to buffa-generated message
  types with no prost interop, forcing a schema-wide migration of
  BES+BEP+RE API protos for a workload that isn't decode-bound, (b)
  doesn't expose the HTTP/2 flow-control knobs phase 30.2 needs to
  tune, and (c) is pre-1.0 with breaking releases ~monthly. See the
  Library / protocol substrate assessment for the full case.
- Not a parallel-stream BES experiment: protocol forbids it.
- Not changing the BEP event schema or which events we emit: those
  are correctness-driven by Plan 18.
