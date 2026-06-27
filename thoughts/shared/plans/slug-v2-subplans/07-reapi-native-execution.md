# Stage 7: REAPI-Native Execution

## Goal

Make REAPI the primary and routine execution boundary for Slug V2.

## Scope

- REAPI `Command`, input tree, CAS upload, `Action`, AC lookup/update,
  `Execute`, output materialization, and evidence logging.
- NativeLink local service bootstrap for local and CI validation.
- actiond as an optional backend behind the same REAPI surface.
- direct-local execution only for narrowly scoped debugging, never as parity
  proof.
- remote cache identity as `ActionDigest -> ActionResult`.

## V1 Extraction Candidates

- Plan 34 NativeLink smoke harness.
- Plan 31 persistent action-cache tests.
- what-ran and what-uploaded evidence patterns.
- materializer and stale-entry handling where it reuses Buck2 RE contracts.
- `app/slug_execute/src/execute/action_digest.rs` and
  `action_digest_and_blobs.rs` as behavior sources for REAPI identity.
- `app/slug_execute_impl/src/executors/re.rs`,
  `app/slug_execute_impl/src/re/download.rs`, and
  `app/slug_execute/src/re/client.rs` as selective rewrite sources.

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

- Port behavior from the Plan 34 harness into Stage 1 oracle fixtures:
  NativeLink config writing, process startup, evidence validation,
  what-ran checks, upload checks, and remote AC-hit assertions.
- NativeLink is the mandatory hosted-Linux baseline for CAS, AC, Execution,
  ByteStream, Capabilities, and WorkerApi.
- Use existing Plan 34 GitHub actions and CI gate tests as shape references,
  but move the V2 proof into `slug-v2-oracle`.
- CI must fail hard if NativeLink setup or the evidence validator is absent on
  hosted Linux.

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
- Extract schema/value semantics and stale-entry behavior from Plan 31
  persistent AC work, but let Stage 3 own V2 output/cache layout.
- Remote AC replay must survive Slug restart and local persistent AC deletion.
- Local durable AC replay must prove SQLite AC short-circuited remote lookup,
  without `CacheQuery` or `Re` evidence.
- Stale local AC and orphaned remote AC entries must re-execute through REAPI
  with zero direct-local fallback.

### 7.6 Ruleset-Shaped REAPI Smoke

- Defer broad ruleset conformance to Stage 8.
- Stage 7 owns REAPI execution proof for C and rules_cc-shaped compile/link
  actions using the existing Plan 34 fixture themes.
- Stage 8 may depend on these fixtures but must not redefine executor-boundary
  evidence.

### 7.7 NativeLink and actiond Local Backends

- NativeLink is the first local REAPI backend. The harness should discover
  `SLUG_V2_NATIVELINK_BIN` first, then a documented sibling checkout path.
- actiond is optional and must sit behind REAPI; no Slug-core actiond shortcut.
- Hosted Linux CI must fail if the configured local REAPI backend is missing
  for mandatory REAPI smoke tests.
- actiond acceptance uses the same Stage 7 evidence with
  `remote_service=local_actiond`, plus supplemental actiond e2e commands.

### 7.8 Evidence Surface

Every REAPI execution fixture must record:

- number of REAPI actions;
- number of direct-local actions, expected zero;
- upload records and aggregate uploaded bytes/digests;
- AC query/update/hit/miss counts;
- output materialization paths and digests;
- backend identity (`nativelink`, hosted, or actiond).
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
- A generated-output fixture uploads inputs, materializes outputs, and can feed
  a downstream action.
- Remote Action Cache hit proof survives Slug daemon restart and local persistent
  cache deletion.
- Hosted CI cannot silently skip the local REAPI proof on Linux.

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
