# Plan 34: REAPI executor boundary and local executor integration

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Historical local-sandbox planning lives in
> [34-sandboxed-execution-strategy-history.md](./34-sandboxed-execution-strategy-history.md).
> Local sandboxing remains useful, but it is an executor-backend concern. This
> active plan owns the Bazel-compatible REAPI execution boundary.

## Goal

Make every supported Slug build/test action execute through the same REAPI
boundary Bazel uses for remote execution: `Command`, input-tree/CAS upload,
`Action`, Action Cache lookup/update, `Execute`, output materialization, and
what-ran accounting.

Local developer execution should be implemented as a local REAPI service, with
NativeLink preferred for the first repo-owned path. Direct in-daemon local spawn
is transitional and cannot count as Plan 34 acceptance. `actiond` integration is
only in scope as a backend behind the REAPI service boundary, not as a Slug-core
shortcut.

## Bazel Source Anchors

- Bazel registers the `remote` spawn strategy through
  `RemoteActionContextProvider`:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteActionContextProvider.java:201-214`.
- The strategy is explicitly a cache plus optional remote-worker execution
  strategy:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteSpawnStrategy.java:20-32`.
- Bazel's gRPC executor issues REAPI `Execute` / `WaitExecution` calls:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/GrpcRemoteExecutor.java:122-177`.

## Current State

- Plan 25 transport config is present in the scrubbed checkout: client snapshot,
  daemon constraint merge, static metadata overlay, and RE-configured executor
  defaulting.
- `get_default_executor_config(..., re_configured = true, ...)` promotes the OSS
  default to `RemoteEnabled(Hybrid, Limited)`, which avoids direct-local-only
  scheduling when an RE backend is configured.
- The active tree does not currently contain a repo-owned NativeLink/local-REAPI
  execution smoke that proves real actions crossed REAPI with
  `direct_local_actions=0`.
- Hybrid/local fallback is still present below the promoted default. Until it is
  quarantined or made to point at a local REAPI service, direct-local success is
  not Plan 34 evidence.

## Accepted Evidence

- `cargo test -p slug_server oss_default_executor_ --lib` proves RE-configured
  OSS defaults are remote-enabled rather than local-only.
- `cargo test -p slug_server re_config_overlay_projects_reapi_executor_snapshot --lib`
  proves the daemon binds an REAPI executor snapshot into static RE metadata.
- `cargo test -p slug_client_ctx cli_re_config_snapshot_projects_bazel_remote_flags --lib`
  proves Bazel-shaped RE flags reach the daemon startup snapshot.

These are boundary invariants, not the final execution proof.

## Remaining Gaps

- Add a fast repo-owned fixture that executes through a local or public REAPI
  executor with:
  - `executor_boundary=reapi`;
  - `direct_local_actions=0`;
  - nonzero `reapi_actions` or equivalent Execute-call evidence;
  - what-ran output showing remote/cache/local counts.
- Prefer NativeLink as the local REAPI service. Use `actiond` only behind that
  REAPI surface if it is needed as the executor backend.
- Quarantine direct-local fallback for RE-configured builds or make it explicit
  diagnostic behavior rather than silent success.
- Keep cache identity and ActionResult replay in Plan 31. Plan 34 only consumes
  cache evidence when it proves the action crossed REAPI.

## Validation Commands

- Focused invariants:
  - `cargo test -p slug_client_ctx cli_re_config_snapshot_projects_bazel_remote_flags --lib`
  - `cargo test -p slug_server re_config_overlay_projects_reapi_executor_snapshot --lib`
  - `cargo test -p slug_server oss_default_executor_ --lib`
- Execution proof still required:
  - run the future NativeLink/local-REAPI fixture and record
    `executor_boundary=reapi`, `direct_local_actions=0`, and what-ran counts.

## Next Owner

Build the NativeLink-local REAPI smoke first. Do not open new flag or
target-language compatibility lanes until at least one fast repo-owned action
executes through REAPI with direct-local action count zero.
