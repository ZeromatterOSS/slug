# Plan 31: Bazel performance parity (warm-RE builds)

> Successor to plan 30 (BES upload throughput). Plan 30's
> 30.1 + 30.2 + 30.5 closed the post-build BES wait gap from 23.2 s
> down to ~2.9 s, dropping slug's default-mode wall on
> `@llvm-project//llvm:llvm` from 57.4 s → 14.81 s. With BES out of the
> way, the remaining gap to bazel is in the build phase itself.

## Goal

Close ≥80% of each gap to bazel on `@llvm-project//llvm:llvm` warm-RE,
measured from `/var/mnt/dev/llvm-project/utils/bazel/`:

| Scenario             | slug (post-30) | bazel  | gap    | target after plan 31 |
|----------------------|----------------|--------|--------|----------------------|
| cold daemon, warm RE | 14.81 s        | 5.92 s | 8.9 s  | ≤ 7.7 s              |
| warm daemon, warm RE | 5.72 s         | 1.03 s | 4.7 s  | ≤ 1.97 s             |

## Background — where the remaining gap lives

### Cold daemon (~8.9 s gap)

bazel's run on the same target reports:
`5884 action cache hit, 1 remote cache hit, 1 internal`. Only 1 of
~5884 action lookups went over the wire. Bazel persists its action
cache to `~/.cache/bazel/_bazel_<user>/<workspace_hash>/action_cache/`
across daemon restarts. Slug's DICE — the type that holds
`ActionKey → ActionResult` — is in-memory only. On a fresh daemon,
every action pays an RE `GetActionResult` round-trip even when the
result is fully cached on the BB.io ActionCache. With ~4853 actions
in the llvm:llvm graph and ~1.7 ms per round-trip, that's the bulk of
slug's `execute=8.2 s` and a large slice of the `materialize=4.6 s`
that follows (each cache hit triggers a download-and-stage step).

A persistent on-disk action cache lifts this onto the local hot path:
SQLite or LMDB lookup at ~10 µs per action × ~4853 = ~50 ms total.
Bazel-equivalent.

### Warm daemon (~4.7 s gap)

Both slug and bazel report `Network: Up: 0B Down: 0B` — pure local
overhead. Three contributors:

1. **Spurious file-watcher invalidation work.** Back-to-back slug runs
   logged `File changed: llvm-project-overlay//bazel-bin/llvm/llvm`
   and `File changed: llvm-project-overlay//slug_warm1.log` — files we
   ourselves wrote, plus shell-redirect output. Each event makes
   `NotifyFileData::sync` insert a `FileChangeTracker` entry that
   DICE re-validates on the next build.

   The existing buck-out filter at `notify.rs:105` is **already** a
   component match (`path.iter().any(|c| c.as_str() == "buck-out")`)
   — the memory note describing it as a prefix bug is stale. The real
   gap is that `bazel-*` convenience symlinks (`bazel-bin`,
   `bazel-out`, `bazel-testlogs`, `bazel-bazel`, `bazel-external`) are
   **not** filtered, so when slug runs in a workspace that has been
   touched by bazel, every action's bazel-bin output looks like a
   source change.

2. **Residual post-build BES wait** in the cold-daemon case (~2 s).
   With plan 30's optimizations, this is now small — but it still
   blocks the client process. Plan 30.3 moves it off the client wall
   entirely. Already scoped in
   `30-bes-upload-throughput.md` §30.3; this plan owns the actual
   landing of it.

## Scope

In:

- 31.1 — Persistent on-disk action cache. New SQLite db at
  `buck-out/v2/cache/action_cache_state`; `ActionDigest` →
  serialized `ActionResult` proto + insertion timestamp.
- 31.2 — File-watcher filter: extend the component-match list with
  the bazel convenience symlinks. Profile what other change events
  remain after that fix to confirm the warm-daemon gap actually
  closes; iterate if not.
- 31.3 — Daemon-resident BES uploader (delivery of plan 30 §30.3).

Out:

- Replacing DICE's in-memory model with on-disk state. Plan 31.1 is
  scoped to the action-cache path only; the DICE generation/version
  storage is untouched.
- A persistent analysis cache. bazel's `--noexperimental_action_cache`
  off-equivalent. Worth a future plan; not this one.
- Watchman-backed file-watching changes. Both watchman and notify
  paths share the buck-out filter logic, but the bazel-symlink fix is
  applied uniformly.
- BES protocol changes (multi-stream, etc.) — already ruled out in
  plan 30.
- Skipping BES connect on no-op builds (was a candidate phase 31.4).
  Bazel always emits BEP events for cache hits and creates an
  invocation page even on no-op builds, so slug's "skip if zero
  actions" would be a divergence from bazel parity. The ~500 ms–1 s
  cost is accepted for now in exchange for UI consistency. Revisit
  only if a parity-friendly path appears (e.g. emitting cache-hit
  BEP events on DICE-cache hits the way bazel does on action-cache
  hits, which is a larger change scoped to a future plan).

## Current State Analysis

### 31.1 — RE ActionCache lookup path

```
DICE BuildKey(ActionKey)  — app/slug_build_api/src/actions/calculation.rs:686
  → build_action_impl                         (line 81)
  → BuckActionExecutor::execute
  → CommandExecutor::action_cache             — app/slug_execute/src/execute/command_executor.rs:124
  → ActionCacheChecker::maybe_execute         — app/slug_execute_impl/src/executors/action_cache.rs:266
  → query_action_cache_and_download_result    (line 74)
  → re_client.action_cache(digest.dupe())     (line 107)
  → RemoteExecutionClientImpl::action_cache   — app/slug_execute/src/re/client.rs:228
  → get_action_result(...)                    (line 977-990)  ← network round-trip
```

Existing on-disk SQLite infra to copy from:

- `app/slug_execute_impl/src/sqlite/materializer_db.rs` — schema v7,
  table at `buck-out/v2/cache/materializer_state`. Holds materializer
  artifact metadata.
- `app/slug_execute_impl/src/sqlite/incremental_state_db.rs` — schema
  v0, table at `buck-out/v2/cache/incremental_state`. Holds
  `(run_action_key, short_path) → content_hash_path` for content-based
  output paths.
- `slug_common::sqlite::sqlite_db::{SqliteDb, SqliteTable, SqliteTables}`
  — generic infrastructure both use. Identity-based corruption
  detection, `slug.sqlite_<name>_version` buckconfig escape hatch.

Action digest type: `ActionDigest = CasDigest<ActionDigestKind>`
(`app/slug_execute/src/execute/action_digest.rs:27`). SHA256 of the
serialized RE `Action` proto, computed in `re_create_action`
(`app/slug_execute/src/execute/command_executor.rs:258`). Globally
deterministic — the same key bazel uses.

`ActionResult` proto is the value type; available from the existing
`re_grpc_proto::build::bazel::remote::execution::v2::ActionResult`.
Serialized via `prost::Message::encode_to_vec`. Typical size: 200–500
bytes per entry; 5000 entries × 400 bytes = ~2 MB SQLite db on disk
for a fully-cached llvm build.

### 31.2 — File watcher

Watcher construction: `NotifyFileWatcher::new` at
`app/slug_file_watcher/src/notify.rs:368`, calls
`install_filtered_watches` at line 277 which uses
`WalkDir::new(root_path).follow_links(false)` and skips symlinks at
the directory level (`is_symlink()` check at line 302).

Component filter: `notify.rs:105` —
`if path.iter().any(|c| c.as_str() == "buck-out") { continue; }`.
The path being checked is a `ProjectRelativePathBuf` (via
`root.relativize(...)` at line 88). Component-match already.

Memory note `file_watcher_buck_out_alias.md` is stale: agent
verification confirmed the recursive-walk symlink-following issue and
the prefix-vs-component mismatch are both already resolved in the
code on disk. Leave the memory note for archaeology, but the bug is
not where the note says.

What's *missing* from the filter: bazel's convenience symlinks
(`bazel-bin`, `bazel-out`, `bazel-testlogs`, `bazel-bazel`,
`bazel-external`). When slug runs in a workspace that bazel also
builds — exactly the case for `@llvm-project//llvm:llvm` benchmark —
each bazel-built artifact in `bazel-bin/` triggers a notify event.

`bazel-external` is a special case: bzlmod stores cached source
archives there for **slug's own use** (created by
`resolve_bzlmod_dependencies` at
`app/slug_common/src/legacy_configs/cells.rs:751`). Filtering it
naively would mask real source changes from external repos. Mitigated
by the fact that `install_filtered_watches` already skips symlinks,
and `bazel-external/<mod>+<ver>` IS a symlink — so we never recurse
into it from notify in the first place. Adding a redundant component
filter is safe.

### 31.3 — Daemon-resident BES (delivery of plan 30 §30.3)

Daemon-side event flow:

```
run_streaming_fallible       — app/slug_server/src/daemon/server.rs:407
  daemon_state.prepare_events(trace_id) → (events, dispatch)   (line 446)
  ServerCommandContext::new(...)                               (line 515)
    runs command body, dispatches via EventDispatcher
  context.finalize().await?                                    (line 531)
  serializes channel → tonic StreamingResponse
```

Per-command lifecycle: `run_server_command` at
`app/slug_server_ctx/src/template.rs:68` wraps every command in a
`span_async(start_event, async { … })` (line 79). `CommandStart` and
`CommandEnd` straddle the body. There is **no** existing daemon-side
per-command subscriber list — only the `EventDispatcher` channel.

Per-invocation state container: `ServerCommandContext` is created per
command and dropped on completion. `DaemonState`
(`app/slug_server/src/daemon/state.rs`) is long-lived (`Arc<DaemonState>`).

For 31.3 / plan 30 §30.3:
- New `DashMap<TraceId, Arc<BesSink>>` on `DaemonState`.
- BES subscriber moves to a daemon-side `EventSubscriber`-equivalent
  hooked into the same `events` source `prepare_events` returns. The
  existing client-side `EventsCtx::handle_events`
  (`app/slug_client_ctx/src/events_ctx.rs:453`) is the structural
  template; daemon side needs an analog that runs concurrently with
  the gRPC serialization.
- Client `BesSubscriber` is removed. Client process exits as soon as
  `BuildFinished` lands; the daemon's BesSink continues to drain.

## Phases

### 31.1 Persistent action cache (largest cold-daemon win)

#### Overview

Add `ActionCacheStateSqliteDb` mirroring `MaterializerStateSqliteDb`.
Lookup runs before the RE call in
`query_action_cache_and_download_result`; insert runs after a
successful RE response.

#### Changes Required

**File**: `app/slug_execute_impl/src/sqlite/action_cache_db.rs` (new)

Modeled on `materializer_db.rs`. Schema:

```sql
CREATE TABLE action_cache (
    digest_hash BLOB NOT NULL,
    digest_size INTEGER NOT NULL,
    -- prost-encoded build.bazel.remote.execution.v2.ActionResult
    action_result BLOB NOT NULL,
    -- unix epoch ms; used for TTL expiry
    cached_at INTEGER NOT NULL,
    PRIMARY KEY (digest_hash, digest_size)
);
CREATE INDEX action_cache_cached_at ON action_cache(cached_at);
```

Schema constant `ACTION_CACHE_DB_SCHEMA_VERSION: u64 = 0`. Bump
buckconfig key: `slug.sqlite_action_cache_state_version`.

**File**: `app/slug_execute_impl/src/sqlite/tables/action_cache_table.rs` (new)

Companion table-helper, modeled on
`tables/materializer_state_table.rs`.

**File**: `app/slug_execute_impl/src/sqlite/mod.rs`

Add `pub mod action_cache_db;`.

**File**: `app/slug_execute_impl/src/executors/action_cache.rs`

Inject `Arc<ActionCacheStateSqliteDb>` into `ActionCacheChecker`. In
`query_action_cache_and_download_result` (line 74), before line 107:

```rust
// Local hit short-circuits the network round-trip.
if cache_type == CacheType::ActionCache {
    if let Some(cached) = local_action_cache.get(&digest)?
        && !cached.is_expired(LOCAL_ACTION_CACHE_TTL)
    {
        // Synthesize an ActionResultResponse and skip the RE call.
        let response = re_grpc_proto::...::ActionResultResponse {
            action_result: cached.action_result,
            ttl_us: cached.remaining_ttl_us(),
        };
        // Fall through to the normal materialization pipeline below.
        let action_cache_response = Ok(Some(response));
        ...
    }
}

let action_cache_response = executor_stage_async(...).await;

// On RE hit, write through to the local cache.
if cache_type == CacheType::ActionCache
    && let Ok(Some(ref resp)) = &action_cache_response
{
    let _ = local_action_cache.put(&digest, &resp.action_result, now_ms());
}
```

`LOCAL_ACTION_CACHE_TTL`: default 6 days (BB.io's known ActionCache
TTL is 7 days; leave a margin). buckconfig override
`slug.local_action_cache_ttl_days`.

**File**: `app/slug_server/src/daemon/state.rs`

Construct the SQLite db once per daemon, pass it through
`prepare_command` into the executor stack. Same wiring pattern as
materializer SQLite (already there).

**File**: `app/slug_execute_impl/src/executors/action_cache.rs`

Add CommandExecution metrics for `local_action_cache_hit`,
`local_action_cache_miss`, `local_action_cache_expired`. Plumb into
the existing `slug_data::CacheQuery` event so the BES feed reports
local hits separately from remote hits. (Bazel reports
`X action cache hit, Y remote cache hit` — same split.)

**Materializer interaction**: a local action-cache hit produces an
`ActionResult`, which the existing `download_action_results` path
(line 198) then materializes via the normal CAS pipeline. The
materializer's own SQLite already tracks what's on disk, so on a
fully-cached run nothing actually re-downloads. The path is
identical to today's "remote ActionCache HIT" path; only the network
call before it changes.

**Eviction + size cap**: bazel caps its disk action cache at
`--max_idle_secs` * write rate, with LRU. We don't need the same
sophistication on day one — the entries are tiny, and bzlmod
invalidation already wipes the materializer SQLite on schema mismatch.
Phase 31.1 ships with TTL-only expiry. Add LRU + size cap (default
500 MB) only if the SQLite grows pathologically in practice.

**Failure modes**:

- Local hit + CAS blob evicted server-side. Materializer download
  fails; we report cache miss and re-execute. Same fallback path as
  today's stale-RE-hit case.
- SQLite corruption. Existing `SqliteDb` infra re-creates the db on
  identity mismatch. Same fail-soft as materializer state.
- TTL race (entry expires mid-build). We snapshot `cached_at` at
  query time; not a correctness issue.

#### Success Criteria

##### Automated Verification:

- [ ] `cargo build -p slug_execute_impl` clean.
- [ ] New unit tests in `action_cache_db.rs`:
      put/get/expire/identity-reset round-trips.
- [ ] Existing test suite passes:
      `slug test fbcode//slug/tests/core/build_command/...`.
- [ ] BES events still report cache hits correctly: in a warm-RE run,
      console shows `Cache hits: 100%` and the BES-side `CacheHit`
      events break down hits into `local` vs `remote`.

##### Manual Verification:

- [ ] **Cold-daemon `@llvm-project//llvm:llvm` warm-RE wall ≤ 7.7 s**
      (target: closes ≥80% of 8.9 s gap).
- [ ] After running once, `buck-out/v2/cache/action_cache_state`
      exists and is non-empty.
- [ ] Killing the daemon, then re-running, hits local action cache
      (visible in console split + much faster `execute` phase).
- [ ] `rm -rf ~/.cache/buildbuddy_cache_test_namespace/` (or
      equivalently invalidate BB's cache for one specific action),
      then re-run: slug detects the CAS-evicted local hit, falls back
      to RE, completes successfully.

---

### 31.2 File-watcher filter for bazel symlinks (largest warm-daemon win)

#### Overview

Extend the component filter at `notify.rs:105` to skip
`bazel-{bin,out,testlogs,bazel,external}` in addition to `buck-out`.
Profile what events remain after the fix and confirm the warm-daemon
gap actually closes; iterate if it doesn't.

#### Changes Required

**File**: `app/slug_file_watcher/src/notify.rs`

Replace the inline buck-out check with a const set of reserved output
directory names:

```rust
/// Path components reserved for build-system output. Files matching
/// any of these in any project-relative path component are filtered
/// out before reaching DICE invalidation. `buck-out` is slug's own
/// output dir; the `bazel-*` entries are bazel's convenience
/// symlinks, which we don't follow into (handled at watch-install
/// time) but still receive aliased notify events for in some FS
/// configurations.
const RESERVED_OUTPUT_COMPONENTS: &[&str] = &[
    "buck-out",
    "bazel-bin",
    "bazel-out",
    "bazel-testlogs",
    "bazel-bazel",
    "bazel-external",
];

if path
    .iter()
    .any(|c| RESERVED_OUTPUT_COMPONENTS.contains(&c.as_str()))
{
    continue;
}
```

**File**: `app/slug_file_watcher/src/watchman/interface.rs`

The watchman path applies its filter via the watchman query itself
(rather than post-hoc); add the same set to the `expression`
`anyof[]` clauses so behavior is uniform across the two backends.

**File**: `app/slug_file_watcher/src/notify.rs` (tests, new)

Add a `#[cfg(test)] mod tests` at the bottom. Drive
`NotifyFileData::process` directly with synthetic events:

```rust
#[test]
fn buck_out_at_root() { /* path "buck-out/v2/foo" → filtered */ }
#[test]
fn buck_out_nested() { /* path "external/cell/buck-out/foo" → filtered */ }
#[test]
fn bazel_bin_at_root() { /* path "bazel-bin/llvm/llvm" → filtered */ }
#[test]
fn bazel_external_nested() { /* path "bazel-external/+llvm_configure+/llvm/foo.cpp" → filtered */ }
#[test]
fn similar_named_source_not_filtered() { /* "src/buckout-tool.rs" passes through */ }
```

#### Profiling step (mandatory before signing off)

After the filter lands, run:

```bash
cd /var/mnt/dev/llvm-project/utils/bazel
slug killall && bazel shutdown
slug build @llvm-project//llvm:llvm --config=remote   # warm
RUST_LOG=slug_file_watcher=debug slug build @llvm-project//llvm:llvm --config=remote 2>watch.log
grep "FileWatcher:" watch.log | sort -u
```

If the surviving event list contains anything that looks like
build-output noise (CI logs, bazel state, etc.), iterate the filter.
If it's just genuine source changes plus an unavoidable handful (e.g.
`.git/index` lock files), document and move on.

If after 31.2 the warm-daemon wall doesn't drop to ≤ 1.97 s, the
remaining cost is in DICE's `update_with_deps` walk on each event,
not the watcher itself — file a follow-up; do not extend 31.2 to
chase it.

#### Success Criteria

##### Automated Verification:

- [x] `cargo test -p slug_file_watcher` passes the new component-match
      tests (5 cases listed above; 6 actually shipped).
- [x] `cargo build -p slug_file_watcher` clean.
- [ ] Existing FS-related core tests pass:
      `slug test fbcode//slug/tests/core/file_watcher/...` if such a
      directory exists, otherwise `tests/e2e/cells/...`.
      (`tests/core/io/` is in OSS `collect_ignore`; not run.)

##### Manual Verification:

- [x] **Warm-daemon `@llvm-project//llvm:llvm` warm-RE wall ≤ 1.97 s**
      (target: closes ≥80% of 4.7 s gap). Median 0.83 s across 5
      trials (vs 5.72 s baseline, 1.03 s bazel). See
      `benchmarks/post-plan-31.2-file-watcher/llvm-project_llvm_llvm/README.md`.
- [x] In `RUST_LOG=slug_file_watcher=debug` output for a back-to-back
      slug run, no `FileWatcher: …bazel-bin…` or
      `FileWatcher: …bazel-out…` lines appear. Verified by running
      slug after an intervening `bazel build`: no bazel-* paths appear
      in user-visible `File changed:` output.
- [ ] In a workspace where a real source file under `bazel-external/`
      changes (i.e. a bzlmod-managed cell archive was updated), the
      change *does* propagate (because `install_filtered_watches`
      didn't recurse into the symlink in the first place — the new
      filter is redundant for that path, not blocking).

---

### 31.3 Daemon-resident BES uploader (delivers plan 30 §30.3)

#### Overview

Move `BesSink` from the client process to the daemon process. Client
exits as soon as `BuildFinished` is observed; daemon's BesSink drains
in the background. Today's `--bes_upload_mode=fully_async` (an alias
for `nowait`) gains its actual meaning.

This is the architectural item from plan 30. Detailed scope already
in `30-bes-upload-throughput.md` §30.3; the changes below pin down
the file-level plumbing.

#### Changes Required

**File**: `app/slug_server/src/daemon/state.rs`

Add a `bes_sinks: DashMap<TraceId, BesSinkHandle>` field. `BesSinkHandle`
wraps `Arc<BesSink>` plus a `JoinHandle<()>` for the upload task.

**File**: `app/slug_server/src/daemon/server.rs`

In `run_streaming_fallible` (line 407), at the same point we have
`trace_id` (line 445), construct the daemon-side BES subscriber and
register it on `bes_sinks`. Drop the client-side `BesSubscriber`
construction.

**File**: `app/slug_server_commands/src/build/bes_subscriber.rs` (new)

Daemon-side equivalent of
`app/slug_client_ctx/src/subscribers/bep_bes_sink.rs`. Same logic,
but consuming directly from the `EventDispatcher` channel rather
than from the gRPC stream. A new trait `DaemonEventSubscriber` (or
reuse a fan-out broadcast channel before tonic serialization) — pick
whichever has the smaller surface area.

**File**: `app/slug_client_ctx/src/subscribers/bep_bes_sink.rs`

Delete the file. Update `app/slug_client_ctx/src/streaming.rs:124`
(the call to `get_bes_subscriber`) to remove the client-side BES
subscriber. Keep `BesSubscriber::log_results_url` (currently at
line 148) — move it to a freestanding `log_bes_results_url` function
in `streaming.rs` since it's the only piece the client still needs.

**File**: `app/slug_build_event_stream/src/grpc_sink.rs`

No code changes; `BesSink` is already daemon-runnable (the only
client-specific bit is `--bes_upload_mode` interpretation, which is
already in `BesConfig`).

**File**: `app/slug_server/src/daemon/state.rs` (graceful shutdown)

On daemon shutdown, drain all `bes_sinks` entries with a timeout
matching `--bes_timeout` (default 60 s). Bazel drops uploads on
server shutdown; we match.

**Flag semantics** (`app/slug_client_ctx/src/common.rs`):

| Mode                         | Client behaviour | Daemon behaviour |
|------------------------------|------------------|------------------|
| `wait_for_upload_complete`   | wait for daemon to ACK upload-complete (default) | drain to completion |
| `nowait`                     | exit on BuildFinished | finish current ACKs only |
| `fully_async`                | exit on BuildFinished | drain to completion in background |
| `--bes_upload_block_client`  | (override) wait                     | drain to completion |

`wait_for_upload_complete` ↔ `fully_async`: in this scheme they
differ only in *who waits*. With the daemon-async default, the
client isn't blocked on BES upload completion under either mode; the
mode just controls whether the daemon drains in the background or
prioritises the next invocation.

#### Success Criteria

##### Automated Verification:

- [ ] `cargo build -p slug_server -p slug_server_commands` clean
      after the move.
- [ ] BES integration tests pass: `tests/core/event_log/...` (if the
      BES subscriber has any), or grep for `BesSubscriber` in the
      test tree.
- [ ] Two back-to-back `slug build` runs complete without daemon
      panic (the second invocation must not race the first's BES
      drain).

##### Manual Verification:

- [ ] Cold-daemon `@llvm-project//llvm:llvm` warm-RE: client wall
      drops by the residual ~2 s post-build BES wait that survived
      plan 30.
- [ ] `slug build … && slug build …` pipeline: second invocation
      starts immediately on first's BuildFinished. Both invocations'
      BB.io pages eventually populate within ~5 s of their respective
      build phases ending.
- [ ] BB.io renders the full Timing tab (chrome trace
      `command.profile.gz`) for a daemon-async run.
- [ ] `slug killall` mid-upload cleanly cancels the in-flight BES
      drain — no client hang, no stale process.

---

## Dependencies and ordering

```
31.1 (action cache, cold-daemon win) ─┐
31.2 (file watcher, warm-daemon win) ─┼─► all three independent;
31.3 (daemon-resident BES)       ─────┘   land in any order
```

Recommended landing order: 31.1 → 31.2 → 31.3.

After 31.1 alone: cold-daemon target hit (~5–6 s).
After 31.2 alone: warm-daemon target hit (~1–2 s).
After 31.3: cold-daemon shaves another ~2 s; client wall ≈ build wall
in all modes.

## Open questions (resolved before plan finalization)

- **Local action-cache TTL.** Default 6 days, buckconfig
  `slug.local_action_cache_ttl_days` overrides. Resolved.
- **Cache-corruption handling.** Reuse `SqliteDb`'s identity-reset
  pattern (drop + recreate on schema/identity mismatch). Resolved.
- **Watchman parity for the bazel-symlink filter.** Apply the same
  list to the watchman query. Resolved (in 31.2 changes).
- **Multi-invocation BES on a single daemon.** Per-invocation
  `DashMap<TraceId, BesSinkHandle>`. Resolved (in 31.3 changes).

## Performance considerations

- **SQLite write amplification (31.1).** Each successful RE cache hit
  writes one ~400-byte row. ~5000 actions = ~2 MB writes per cold
  build, dominated by random-page IO on rotational disks. Use
  `INSERT OR REPLACE INTO ... VALUES (...)` batched at 256-row
  transactions; same pattern as `materializer_db`'s update path.
- **DICE invalidation cost (31.2).** Even with the filter, real
  source changes still trigger DICE re-eval. The filter only removes
  *spurious* events; the warm-daemon win is bounded by how many of
  the observed events were actually spurious. The benchmark logged
  `bazel-bin/llvm/llvm` and `bazel-bin/llvm/libSupport.a` (both real
  bazel artifacts under llvm-project) so the filter alone should
  close most of the gap.
- **Daemon memory (31.3).** A retained BesSink per concurrent
  invocation is a few MB. With typical CI/dev concurrency ≤ 4, this
  is negligible.

## Migration notes

- 31.1 introduces a new SQLite db. First daemon to see the new
  binary creates it empty. No data migration needed.
- 31.2 changes filter behaviour. No on-disk state change. A workspace
  with `bazel-bin/` that previously triggered cache invalidation will
  now ignore those events — desired.
- 31.3 removes a client-side subscriber. Old clients talking to a new
  daemon: client-side BES subscriber is gone (no double-uploads). New
  clients talking to an old daemon: graceful — the daemon just won't
  do anything BES-related, client falls back to its own. Plan a
  bump-then-deprecate path: ship 31.3 with both paths active behind a
  daemon capability flag, remove the client path in a follow-up.

## What this plan is NOT

- Not a DICE persistence rework. The action cache stays in DICE for
  in-memory hits; only the cross-daemon persistence layer is added.
- Not a watchman migration or replacement.
- Not a switch to a different RPC stack. Plan 30 already ruled out
  connect-rust + buffa; same conclusions hold here.
- Not a parallel-stream BES experiment. Protocol forbids it.
- Not a benchmark suite overhaul. Plan 31 inherits plan 30's
  methodology; the benchmark dir at
  `benchmarks/post-plan-30-bes-throughput/llvm-project_llvm_llvm/`
  is the comparison point.

## References

- Plan 30 (BES upload throughput): `30-bes-upload-throughput.md`
  — establishes the baseline this plan builds on; §30.3 is the
  origin of plan 31.3.
- Plan 30 benchmark dir:
  `/var/mnt/dev/slug/benchmarks/post-plan-30-bes-throughput/llvm-project_llvm_llvm/README.md`
  — the slug-vs-bazel numbers this plan targets closing.
- Action cache lookup site:
  `app/slug_execute_impl/src/executors/action_cache.rs:107`.
- File-watcher buck-out filter:
  `app/slug_file_watcher/src/notify.rs:105` (already
  component-match; the memory note at
  `~/.claude/projects/-var-mnt-dev-slug/memory/file_watcher_buck_out_alias.md`
  is stale).
- Daemon command lifecycle:
  `app/slug_server_ctx/src/template.rs:68`.
- BES finalize path:
  `app/slug_client_ctx/src/subscribers/bep_bes_sink.rs:238–327`.
