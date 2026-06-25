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
- `tests/plan34/test_reapi_local_executor_smoke.py` is an opt-in repo-owned
  smoke. When `SLUG_PLAN34_NATIVELINK_BIN` points at a NativeLink binary, it
  starts a local all-in-one NativeLink REAPI service with one worker and builds
  fast in-repo fixtures through `--remote_executor`, `--remote_cache`, and
  `--remote-only`.
- The smoke proves both a one-action shell fixture and a three-action C-source
  Starlark rule fixture cross REAPI with what-ran `executor="Re"` and zero
  direct-local actions. It remains opt-in because NativeLink binary/bootstrap
  availability is not yet repo-owned or a CI gate.
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
- `SLUG_PLAN34_NATIVELINK_BIN=/path/to/nativelink SLUG_BIN=target/debug/slug python -m pytest tests/plan34/test_reapi_local_executor_smoke.py -q -s`
  proves local NativeLink-backed REAPI execution for repo-owned fixtures with
  `reapi_actions=1` for the shell fixture, `reapi_actions=3` for the C-source
  Starlark rule fixture, `direct_local_actions=0`, and local command count 0.

The NativeLink smoke is execution-boundary evidence, but it is not yet a
routine gate or full `@rules_cc` toolchain proof.

## Remaining Gaps

- Promote the NativeLink-local REAPI smoke from opt-in to routine gate once
  NativeLink binary/config bootstrap is repo-owned or otherwise available in CI.
- Promote the C-source fixture from compile/link-shaped rule actions to full
  `@rules_cc` once the Bazel 9 C++ toolchain registration path executes without
  falling back to direct local.
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
- NativeLink/local-REAPI execution proof:
  - `SLUG_PLAN34_NATIVELINK_BIN=/path/to/nativelink SLUG_BIN=target/debug/slug python -m pytest tests/plan34/test_reapi_local_executor_smoke.py -q -s`

## Next Owner

Promote the NativeLink-local REAPI smoke into a repeatable gate, then broaden it
to a fast cc/rules fixture. Do not open new flag or target-language
compatibility lanes until the local REAPI path is routine and direct-local
fallback is quarantined for RE-configured builds.
