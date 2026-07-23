# Stage 7: REAPI-Native Execution

## Goal

Make REAPI the primary and routine execution boundary for Slug V2.

## Scope

- REAPI `Command`, input tree, CAS upload, `Action`, AC lookup/update,
  `Execute`, output materialization, and evidence logging.
- BuildBuddy as the primary scaled remote build/cache lane for Bazel-built and
  Slug-built development/CI workloads.
- sibling `../actiond` as the preferred local REAPI conformance backend, with
  NativeLink retained for already-landed regression coverage.
- direct-local execution only for narrowly scoped debugging, never as parity
  proof.
- remote cache identity as `ActionDigest -> ActionResult`.
- no Slug-local sandbox implementation until the post-aquery execution/cache
  gate is stable.

## Current Priority Hold After Aquery

The landed NativeLink write/shell action proofs remain required regressions,
but new Stage 7 breadth does not control the next milestone. Freeze new cache,
materializer, retry/TLS, and backend feature work unless it:

- preserves an already-landed regression;
- enables the Stage 10 Bazel/BuildBuddy developer build without changing Slug
  execution semantics; or
- begins after Stage 8 has accepted exact `aquery` for the gate matrix.

Once the hold lifts, consume the exact Stage 6/`aquery` action objects. Do not
maintain a second executor-only action description that can drift from query
output.

## V1 Extraction Candidates

- Plan 34 design and NativeLink smoke harness in
  `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/34-sandboxed-execution-strategy.md`,
  `slug-v1-archive:tests/plan34/test_reapi_local_executor_smoke.py`, and
  `slug-v1-archive:tests/plan34/validate_reapi_evidence.py`.
- Plan 31 persistent action-cache design and tests in
  `slug-v1-archive:thoughts/shared/plans/slug-bazel-subplans/31-bazel-perf-parity.md`
  and `slug-v1-archive:tests/plan31/test_persistent_re_action_cache.py`.
- what-ran and what-uploaded evidence patterns from the archived Plan 34 smoke
  and validator paths above.
- materializer and stale-entry handling from
  `slug-v1-archive:app/slug_execute_impl/src/materializers/deferred/io_handler.rs`
  and the archived Plan 31 test above where they reuse Buck2 RE contracts.
- `slug-v1-archive:app/slug_execute/src/execute/action_digest.rs` and
  `slug-v1-archive:app/slug_execute/src/execute/action_digest_and_blobs.rs` as
  behavior sources for REAPI identity.
- `slug-v1-archive:app/slug_execute_impl/src/executors/re.rs`,
  `slug-v1-archive:app/slug_execute_impl/src/re/download.rs`, and
  `slug-v1-archive:app/slug_execute/src/re/client.rs` as selective rewrite
  sources.

These paths are absent from the active clean root. Inspect them with
`git show slug-v1-archive:<path>` or an external archive worktree; do not search
for or import them from the active root. Use the matching
[Stage 9 extraction-ledger](./09-v1-extraction-ledger.md) row to choose the
import mode, oracle, and validation.

## Bazel Oracle Anchors

- `RemoteActionContextProvider.java` registers the remote strategy and spawn
  cache.
- `RemoteModule.java` owns remote flags, endpoint semantics, cache-only versus
  execution configuration, headers, TLS, and instance names.
- `RemoteOptions.java` owns option names and defaults.
- `RemoteSpawnStrategy.java`, `RemoteSpawnRunner.java`, `RemoteExecutionService.java`,
  `GrpcRemoteExecutor.java`, and `GrpcCacheClient.java` own REAPI execution,
  cache, and materialization behavior.
- Remote shell tests under `src/test/shell/bazel/remote/` are the first oracle
  for tag/no-cache, TLS/header, combined cache, and Action Cache edge cases.

## Implementation Slices

### 7.1 NativeLink REAPI Harness

- Port behavior from
  `slug-v1-archive:tests/plan34/test_reapi_local_executor_smoke.py` into Stage 1
  oracle fixtures: NativeLink config writing, process startup, evidence
  validation, what-ran checks, upload checks, and remote AC-hit assertions.
- NativeLink is the retained baseline for the already-landed CAS, AC,
  Execution, ByteStream, Capabilities, and WorkerApi fixtures. New scaled CI
  proof uses BuildBuddy and new local backend conformance should also run
  against actiond.
- Use
  `slug-v1-archive:.github/actions/setup_plan34_nativelink/action.yml`,
  `slug-v1-archive:.github/actions/run_plan34_reapi/action.yml`,
  `slug-v1-archive:.github/workflows/plan34-reapi.yml`, and
  `slug-v1-archive:tests/plan34/test_ci_gate.py` as shape references, but move
  the V2 proof into `slug-v2-oracle`.
- A lane that declares NativeLink conformance must fail hard if setup or the
  evidence validator is absent; it is not mandatory for unrelated lanes.

### 7.2 Remote Configuration

- Parse `--remote_executor`, `--remote_cache`, remote headers,
  `--remote_instance_name`, TLS options, remote timeout/retry options, and
  default exec properties.
- Bare `--remote_executor` supplies CAS/AC endpoint unless overridden by
  `--remote_cache`, matching Bazel.
- Cache-only configuration must not imply remote execution.

### 7.3 Action IR to REAPI

- Convert Stage 6 action IR into REAPI `Command`, input tree, `Action`, and
  Platform properties.
- Environment handling follows Bazel 9 default shell env and `--action_env`.
- Paramfiles and tree artifacts are represented in the uploaded input tree, not
  hidden local files.
- Rebuild V2 equivalents of REAPI action digest/blob assembly around Bazel
  action declarations. Do not carry Buck path spelling into identity.
- Identity is `Command` digest plus input-root digest plus platform and timeout
  fields exactly as REAPI `Action` encodes them.
- Digest serialized REAPI protobuf messages and construct a real `Directory`
  Merkle tree. Rust `Debug` formatting, ad hoc text serialization, a flat input
  list, or a separately invented platform digest are never action identity.
- Selectively port the design from
  `slug-v1-archive:app/slug_execute/src/execute/action_digest_and_blobs.rs`
  behind V2-owned action types; preserve the protobuf and CAS contract while
  rejecting Buck executor configuration and path semantics.

### 7.4 CAS Upload, Execute, and Materialize

- Implement digest-first upload with `FindMissingBlobs`, batched uploads, and
  upload dedupe.
- Materialization verifies digest/size and treats corrupt local blobs as
  missing.
- Generated outputs can be re-uploaded for downstream RE actions if the remote
  CAS reports them missing.
- Required fixtures cover one shell action, bare `--remote_executor` with no
  explicit cache, platform exec properties, nested paramfile input trees, and a
  generated output consumed by a downstream action.

### 7.5 Durable Action Cache

- Cache identity is REAPI `ActionDigest -> ActionResult`.
- AC lookup, stale-entry handling, download, and cache-hit accounting are
  separate testable steps.
- Do not count copied-output bridges or direct-local fallbacks as cache success.
- Extract schema/value semantics and stale-entry behavior from
  `slug-v1-archive:app/slug_execute_impl/src/sqlite/action_cache_db.rs`,
  `slug-v1-archive:app/slug_execute_impl/src/sqlite/tables/action_cache_table.rs`,
  and `slug-v1-archive:tests/plan31/test_persistent_re_action_cache.py`, but let
  Stage 3 own V2 output/cache layout.
- Remote AC replay must survive Slug restart and local persistent AC deletion.
- Local durable AC replay must prove SQLite AC short-circuited remote lookup,
  without `CacheQuery` or `Re` evidence.
- Stale local AC and orphaned remote AC entries must re-execute through REAPI
  with zero direct-local fallback.

### 7.6 Ruleset-Shaped REAPI Smoke

- Defer broad ruleset conformance to Stage 8.
- Stage 7 owns REAPI execution proof for C and rules_cc-shaped compile/link
  actions using the archived
  `slug-v1-archive:tests/plan34/fixtures/cc_actions/BUILD.bazel`,
  `slug-v1-archive:tests/plan34/fixtures/cc_actions/defs.bzl`, and
  `slug-v1-archive:tests/plan34/fixtures/rules_cc/BUILD.bazel` fixture themes.
- Stage 8 may depend on these fixtures but must not redefine executor-boundary
  evidence.

### 7.7 BuildBuddy, actiond, and NativeLink Backends

- BuildBuddy is the primary hosted/scaled backend. Repository configuration may
  name endpoints and non-secret options; credentials remain in the user's
  `~/.bazelrc` or injected CI secrets and must never be read, copied into
  evidence, or committed.
- actiond at sibling commit
  `ca39423bbd78916457f3225dcab826283c18f412` is the preferred local REAPI
  testbed. It must sit behind REAPI with no Slug-core shortcut. Its acceptance
  uses the same evidence with `remote_service=local_actiond` plus its own
  focused e2e health command.
- NativeLink remains a supported regression backend for the existing fixture
  chain. The harness may discover `SLUG_V2_NATIVELINK_BIN` and a documented
  sibling path, but future design must not be coupled to NativeLink-specific
  worker APIs.
- Every mandatory backend lane fails hard when its configured service or
  evidence validator is absent; optional lanes report skipped without being
  counted as parity proof.
- Isolation implemented inside actiond or BuildBuddy is backend behavior. Slug
  sandboxing remains explicitly deferred rather than being inferred from a
  successful remote action.

### 7.8 Evidence Surface

Every REAPI execution fixture must record:

- number of REAPI actions;
- number of direct-local actions, expected zero;
- upload records and aggregate uploaded bytes/digests;
- AC query/update/hit/miss counts;
- output materialization paths and digests;
- backend identity (`buildbuddy`, `actiond`, or `nativelink`).
- `executor_boundary: "reapi"` for every execution row.
- nonempty action digests and expected platform properties.
- no `Local`, `LocalWorker`, `Worker`, or `WorkerInit` what-ran entries.

## Exact Test Criteria

- `shell-action-reapi` executes one action through NativeLink with
  `reapi_actions=1`, `direct_local_actions=0`, one upload record, and a
  materialized declared output digest.
- `bare-remote-executor-reapi` omits `--remote_cache` and still uploads
  CAS/AC through the executor endpoint.
- `platform-exec-properties-reapi` compares Platform properties in evidence.
- `remote-cache-only-no-execute` sets only `--remote_cache`, proves AC/CAS are
  used, and proves no remote execution is attempted.
- `reapi-action-cache-hit` primes, kills Slug daemon, deletes local persistent
  AC state, rebuilds, and observes AC hit with `remote=0`, `local=0`.
- `reapi-local-action-cache-hit` proves durable local AC hit without remote
  lookup.
- `reapi-stale-local-ac-reexec` and `reapi-orphaned-remote-ac-reexec` re-execute
  through REAPI with zero direct-local fallback.
- `reapi-generated-output-reupload` runs producer and consumer RE actions,
  clears remote CAS for the produced output where the backend allows, and proves
  Slug re-uploads before consumer execution.
- `reapi-paramfile-input-tree` proves nested paramfiles are part of the uploaded
  input tree.
- REAPI identity tests compare serialized `Command`, `Directory`, and `Action`
  digests against the Bazel oracle or a directly equivalent protobuf fixture;
  text/debug serialization is forbidden by regression coverage.
- `rules-cc-reapi-basic` proves compile/link-shaped actions cross the REAPI
  boundary before Stage 8 ruleset breadth.
- `tag-no-remote-cache` and `tag-no-cache` fixtures match Bazel remote test
  semantics for cache suppression.
- Remote execution phase evidence must show `Commands: N (cached: 0, remote:
  N, local: 0)`.
- Remote AC replay evidence must show `cached: N, remote: 0, local: 0` with
  matching `CacheQuery` and `Cache` digests.

## Acceptance Criteria

- A one-action shell fixture executes through NativeLink REAPI with zero
  direct-local actions.
- Action and input-root digests come from REAPI protobuf/Merkle serialization
  and are accepted by the configured REAPI backend.
- A generated-output fixture uploads inputs, materializes outputs, and can feed
  a downstream action.
- Remote Action Cache hit proof survives Slug daemon restart and local persistent
  cache deletion.
- Hosted CI cannot silently skip the local REAPI proof on Linux.
- The accepted matrix includes BuildBuddy for scaled RBE/cache and actiond for
  local REAPI conformance; NativeLink continues to run the historical
  regression subset.
- The protobuf action shown by accepted `aquery` is the action uploaded to
  CAS/Execution, modulo only REAPI envelope fields that Stage 7 owns and tests.

## Validation

```bash
slug-v2-oracle run --fixture shell-action-reapi
slug-v2-oracle run --fixture bare-remote-executor-reapi
slug-v2-oracle run --fixture platform-exec-properties-reapi
slug-v2-oracle run --fixture remote-cache-only-no-execute
slug-v2-oracle run --fixture reapi-action-cache-hit
slug-v2-oracle run --fixture reapi-local-action-cache-hit
slug-v2-oracle run --fixture reapi-stale-local-ac-reexec
slug-v2-oracle run --fixture reapi-orphaned-remote-ac-reexec
slug-v2-oracle run --fixture reapi-generated-output-reupload
slug-v2-oracle run --fixture reapi-paramfile-input-tree
slug-v2-oracle run --fixture rules-cc-reapi-basic
slug-v2-oracle run --fixture tag-no-remote-cache
slug-v2-oracle run --fixture tag-no-cache
slug-v2-oracle validate-evidence /path/to/evidence.jsonl
```
## Checkpoint Evidence

- 2026-06-27 Stage 7.2/7.3 REAPI substrate: added `slug_reapi_v2`
  with remote executor/cache config parsing, Bazel-shaped bare
  `--remote_executor` cache endpoint semantics, cache-only mode distinction,
  remote headers/instance/default exec properties, SHA-256 REAPI digest helper,
  Stage 6 action IR to REAPI command projection, action identity serialization,
  and evidence rows pinning `executor_boundary = "reapi"` with zero
  direct-local actions. Added oracle fixture scaffolds `shell-action-reapi`,
  `bare-remote-executor-reapi`, `platform-exec-properties-reapi`, and
  `remote-cache-only-no-execute`.
  Validation: `cargo test -p slug_reapi_v2`; `py -3 -B tools/v2_oracle list`;
  `rg -n "direct-local|DirectLocal|LocalWorker|buck-out|CellResolver|process-global"
  app/slug_reapi_v2 app/slug_analysis_v2 app/slug_build_api_v2` returned no
  matches. NativeLink startup, CAS upload, Execute, AC, and materialization are
  not implemented yet, so Stage 7 oracle execution remains skipped.
- 2026-06-27 Stage 7.8 evidence validator: added
  `tools/v2_oracle validate-evidence` and `tools/v2_oracle_lib/evidence.py`
  to validate REAPI JSONL rows for `executor_boundary = "reapi"`, positive
  REAPI action counts, zero direct-local actions, backend identity, nonempty
  action/upload/materialized digests, and absence of forbidden local what-ran
  entries. Validation: `py -3 -B tools/v2_oracle validate-evidence
  target/v2_oracle_evidence_smoke.jsonl`; bundled Python `pytest -q -p
  no:cacheprovider tests/v2_oracle/test_v2_oracle.py` passed 11 tests.
- 2026-06-27 Stage 7.3/7.4 input tree and CAS planning substrate: added
  REAPI input tree assembly for action inputs, tools, and paramfiles, digest
  parsing/validation, deterministic input-root digests, digest-first CAS upload
  planning, and generated-output reupload planning. Added oracle fixture
  scaffolds `reapi-paramfile-input-tree` and
  `reapi-generated-output-reupload`. Validation: `cargo test -p
  slug_reapi_v2` passed 8 tests; `py -3 -B tools/v2_oracle list`; `rg -n
  "direct-local|DirectLocal|LocalWorker|buck-out|CellResolver|process-global"
  app/slug_reapi_v2 app/slug_analysis_v2 app/slug_build_api_v2` returned no
  matches. NativeLink-backed `FindMissingBlobs`, upload, Execute,
  materialization, and AC replay remain pending.
- 2026-06-27 Stage 7.5 action-cache substrate: added
  `ActionDigest -> ActionResult` records, action-cache lookup table semantics,
  local materialized-output stale detection, and remote-CAS orphaned-output
  detection. Added fixture scaffolds `reapi-action-cache-hit`,
  `reapi-local-action-cache-hit`, `reapi-stale-local-ac-reexec`, and
  `reapi-orphaned-remote-ac-reexec`. Validation: `cargo test -p
  slug_reapi_v2` passed 11 tests; `py -3 -B tools/v2_oracle list`; `rg -n
  "direct-local|DirectLocal|LocalWorker|buck-out|CellResolver|process-global"
  app/slug_reapi_v2 app/slug_analysis_v2 app/slug_build_api_v2` returned no
  matches. Durable SQLite persistence, remote AC service calls, daemon restart
  replay, and REAPI re-execution still require the Stage 7 backend harness.
- 2026-07-14 Stage 7.3 protobuf/Merkle correction: replaced the provisional
  debug/text command and flat-input identity with a wire-compatible REAPI v2
  subset. `Command` and `Action` are encoded with their protocol field
  numbers; `Action` owns the command, input-root, platform, and timeout fields.
  Input paths now build canonical nested `Directory` messages, ordered by path,
  and retain every directory plus inline-paramfile blob for CAS upload. This
  follows Bazel `third_party/remoteapis/build/bazel/remote/execution/v2/
  remote_execution.proto` (`Action`, `Command`, `Directory`, `Platform`), and
  the blob-assembly boundary in
  `slug-v1-archive:app/slug_execute/src/execute/action_digest_and_blobs.rs`.
  Validation: `CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target
  CARGO_BUILD_JOBS=1 cargo test -p slug_reapi_v2 --no-fail-fast` passed 11
  tests, including decoded Action fields and a nested `pkg`/`tools` Directory
  regression; `cargo fmt --check`; `git diff --check`. This is only the
  identity/CAS-object substrate: `FindMissingBlobs`, upload, Execute,
  NativeLink acceptance, materialization, and AC service calls are still open.
- 2026-07-14 Stage 7.3/7.4 first NativeLink execution slice: generated the
  narrow REAPI v2 CAS/Execution client from a checked-in projection of Bazel's
  protocol, then added V2-owned missing-blob discovery, batch upload, Execute
  stream decoding, output download, digest verification, and safe
  `bazel-bin` materialization. Declarative `ctx.actions.write` actions lower
  to an uploaded shell command rather than running locally. The one-shot
  `slug build --remote_executor=...` boundary now executes declared actions
  through this client and reports REAPI evidence. Validation: a real
  `../nativelink/target/release/nativelink` process using its local
  CAS/AC/scheduler/worker configuration passed the ignored focused test
  `native_link_executes_uploaded_write_action_and_materializes_output` with
  one REAPI action, zero direct-local actions, and the materialized output
  bytes. `cargo test -p slug_reapi_v2 --no-fail-fast` passed 13 non-backend
  tests; `cargo check -p slug_cli_v2` passed. Remaining work is Stage 1 oracle
  integration, request headers/TLS/retries, output-directory/tree handling,
  durable AC replay, and same-daemon invalidation.
- 2026-07-14 Stage 7.8 oracle integration (gate clause 4): the
  `simple-rule-action` fixture now drives the full REAPI boundary through the
  Stage 1 oracle harness. Added `tools/v2_oracle_lib/nativelink.py` with
  NativeLink binary discovery (`SLUG_V2_NATIVELINK_BIN` then sibling
  `../nativelink/target/{release,smol,debug}/nativelink`), local CAS/AC/
  scheduler/worker config generation, startup readiness polling, and teardown.
  The fixture parser gained a `[reapi]` section; when `remote_executor = true`
  and the tool is slug, the runner starts NativeLink, appends
  `--remote_executor=<endpoint>` (plus declared `default_exec_properties`) to
  the slug argv, extracts the REAPI evidence JSON from stderr, and tears down
  NativeLink in a `finally` block. The build command now emits valid JSON with
  `action_digests`, `uploaded_digests`, and `materialized_outputs` digest
  lists; the comparison layer validates `reapi_actions >= 1`,
  `direct_local_actions == 0`, and nonempty digest lists for slug+reapi runs.
  Materialized outputs are now written with mode `0o555` to match Bazel's
  read-only action-output policy. The Bazel oracle (generated with Bazel 9.1.1)
  compares the declared output manifest and digest exactly. Validation:
  `python3 -B -m tools.v2_oracle run --fixture simple-rule-action --tool slug
  --slug <slug-v2-bin> --timeout 60` reported `status: ok` with
  `reapi_actions=1`, `direct_local_actions=0`, materialized output digest
  `dc5b456bbed0dafb1a5719d46d4484453b730745b12083e67b240c953e427a49/21`
  matching the checked-in Bazel oracle; `CARGO_BUILD_JOBS=1 cargo test -p
  slug_cli_v2 -p slug_reapi_v2 --no-fail-fast` passed 20 tests (1 ignored);
  `python3.12 -B -m pytest -q -p no:cacheprovider
  tests/v2_oracle/test_v2_oracle.py` passed 17 tests; `cargo fmt --check`;
  forbidden-surface grep unchanged. Remaining Stage 7 work: headers/TLS/
  retries, output-directory/tree handling, durable AC replay, and same-daemon
  invalidation.
- 2026-07-14 Stage 7.9 run_shell execution (gate clause 4, second fixture):
  the `shell-action-reapi` fixture drives `ctx.actions.run_shell` through the
  full REAPI boundary. Added the `ctx.actions.run_shell(outputs, command,
  arguments)` Starlark binding and a `path` property on declared files
  (`get_attr`/`has_attr` on `DeclaredFile`, not a method, because starlark-rust
  0.13 method lookup shadows `get_attr`). Fixed the shell argv pad bug: Bazel's
  `ShellCommand` (StarlarkActionFactory.java:627 "add an empty argument before
  other arguments") inserts an empty `$0` when arguments are present so the
  first user argument is `$1`; `CtxActions::run_shell` now matches this `pad`
  behavior. Citation: `src/main/java/.../analysis/actions/ShellCommand.java:46`
  and `.../starlark/StarlarkActionFactory.java:631`. The `RunShell` action
  lowers through the existing argv path in `ReapiCommand::for_execution` (no
  new lowering needed since `run_shell` sets argv directly). Generated the
  Bazel 9.2.0 oracle with `--remote_executor` against NativeLink 1.4.0 (local
  CAS/AC/scheduler/worker); Bazel also succeeds remotely, confirming the worker
  creates output parent directories from declared `output_files`. Validation:
  `python3 -B -m tools.v2_oracle run --fixture shell-action-reapi --tool slug
  --slug <slug-v2-bin> --timeout 60` reported `status: ok` with
  `reapi_actions=1`, `direct_local_actions=0`, materialized output digest
  `ac0cb855e0243634730f146e7b14a0dbc8ed0c3271e7b6ca4974c116a87f2a28/5`
  matching the checked-in Bazel oracle; `CARGO_BUILD_JOBS=1 cargo test -p
  slug_cli_v2 -p slug_reapi_v2 -p slug_analysis_v2 -p slug_build_api_v2
  --no-fail-fast` passed 49 tests (1 ignored); `python3.12 -B -m pytest -q -p
  no:cacheprovider tests/v2_oracle/test_v2_oracle.py` passed 17 tests; `cargo
  fmt --check`. Remaining Stage 7 work: headers/TLS/retries,
  output-directory/tree handling, durable AC replay, and same-daemon
  invalidation.
- 2026-07-14 Stage 7.10 bare-executor and platform-properties fixtures:
  converted the `bare-remote-executor-reapi` and
  `platform-exec-properties-reapi` placeholder fixtures to live oracle
  fixtures. Both now use the harness `[reapi]` section (no hardcoded ports).
  `bare-remote-executor-reapi` proves that bare `--remote_executor` (no
  `--remote_cache`) supplies CAS/AC; output digest
  `ac0cb855e0243634730f146e7b14a0dbc8ed0c3271e7b6ca4974c116a87f2a28/5`
  matches Bazel 9.2.0. `platform-exec-properties-reapi` exercises Platform
  properties: the fixture declares `default_exec_properties` (sent to slug as
  `--remote_default_exec_properties`) and `worker_platform_properties`
  (injected into the NativeLink scheduler `supported_platform_properties` +
  worker `platform_properties`). NativeLink 1.4.0 config detail: string
  properties need `PropertyType::Exact` (not `Minimum`, which is u64-only), and
  keys with hyphens must be quoted in JSON5 (`"container-image"`). Slug now
  emits `platform_properties` in the REAPI evidence JSON, and the comparison
  layer validates that each declared property appears in the evidence.
  Citation: `nativelink-config/src/schedulers.rs:43` (`PropertyType` enum).
  Validation: both fixtures report `status: ok` through the oracle harness;
  `CARGO_BUILD_JOBS=1 cargo test -p slug_cli_v2 -p slug_reapi_v2 -p
  slug_analysis_v2 -p slug_build_api_v2 --no-fail-fast` passed 49 tests (1
  ignored); `python3.12 -B -m pytest -q -p no:cacheprovider
  tests/v2_oracle/test_v2_oracle.py` passed 20 tests (3 new: platform-property
  parser + reject/accept evidence); `cargo fmt --check`. Four REAPI fixtures
  now pass: `simple-rule-action`, `shell-action-reapi`,
  `bare-remote-executor-reapi`, `platform-exec-properties-reapi`. Remaining
  Stage 7 work: headers/TLS/retries, output-directory/tree handling, durable
  AC replay.
- 2026-07-16 Stage 7.11 same-daemon DICE invalidation (gate clause 5): the
  `load-invalidation` fixture now passes end-to-end through the oracle
  harness. Introduced `slug_server_v2` with a `Daemon` that retains a
  `BzlModuleEvaluator` + file-digest cache across builds. Before each build
  the daemon rescans `.bzl`/`BUILD.bazel` files, compares SHA-256 digests,
  and calls `invalidate_path`/`invalidate_package` for changed paths; the
  DICE graph replays only the affected computations. The CLI gains
  `--output_base` startup-flag parsing and auto-starts the daemon via Unix
  socket when set; `--serve` mode re-execs the binary as the server. The
  harness gains a `daemon` fixture flag that passes `--output_base` to slug
  and shuts down the daemon after all commands. Fixed: stale read-only
  outputs (0o555) blocked same-daemon rebuilds — `materialize_outputs` now
  removes stale files before writing. Regenerated the Bazel 9.2.0 oracle
  (stale `0o444` mode → correct `0o555`). Validation: `python3 -B -m
  tools.v2_oracle run --fixture load-invalidation --tool slug --slug
  <slug-v2-bin> --timeout 60` reported `status: ok`; prime digest
  `2c8b08da.../4`, after_bzl_edit digest `27dd8ed4.../4` (1 file
  invalidated), both mode `0o555`, matching the Bazel oracle;
  `CARGO_BUILD_JOBS=1 cargo test -p slug_server_v2 -p slug_cli_v2 -p
  slug_reapi_v2 -p slug_core_v2 -p slug_analysis_v2 -p slug_build_api_v2
  --no-fail-fast` passed 56 tests (1 ignored); `python3.12 -B -m pytest -q
  -p no:cacheprovider tests/v2_oracle/test_v2_oracle.py` passed 22 tests;
  `cargo fmt --check`. All five gate clauses now have passing fixtures.
  Remaining Stage 7 work: headers/TLS/retries, output-directory/tree
  handling, durable AC replay.
- 2026-07-16 Stage 7.12 action-cache hit (GetActionResult client lookup):
  added the REAPI `ActionCache` service with `GetActionResult` to
  `reapi_v2.proto` (message `GetActionResultRequest` with instance_name,
  action_digest, inline_stdout/stderr/inline_output_files). The executor
  now calls `GetActionResult` before `Execute`: a hit returns the cached
  `ActionResult` and skips the execution server entirely; a miss falls
  through to the existing `Execute` path (extracted into
  `execute_through_server`). Evidence distinguishes the two:
  `ac_hits=1, uploaded=0` on a hit vs `ac_misses=1, uploaded>0` on a miss.
  The `reapi-action-cache-hit` fixture (daemon mode, two builds) passes:
  prime reports `ac_misses=1` with 2 uploads; replay reports `ac_hits=1`
  with 0 uploads. The comparison layer now requires `uploaded_digests`
  only on AC misses (hits legitimately have no uploads). Citation:
  `third_party/remoteapis/build/bazel/remote/execution/v2/
  remote_execution.proto` (`ActionCache.GetActionResult`). Validation:
  `python3 -B -m tools.v2_oracle run --fixture reapi-action-cache-hit
  --tool slug` reported `status: ok`; `CARGO_BUILD_JOBS=1 cargo test -p
  slug_reapi_v2 -p slug_cli_v2 --no-fail-fast` passed 20 tests (1
  ignored); `python3.12 -B -m pytest -q -p no:cacheprovider
  tests/v2_oracle/test_v2_oracle.py` passed 24 tests (2 new AC-evidence
  comparison tests); `cargo fmt --check`.
