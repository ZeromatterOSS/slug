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
  default to `RemoteEnabled(Remote)`, so Bazel-shaped `--remote_executor`
  builds use REAPI by default instead of silently falling back to direct-local
  execution.
- Bazel `platform(exec_properties = {...})` handling now also synthesizes
  `RemoteEnabled(Remote)` for the platform-derived executor config. Label-shaped
  build-setting keys stay out of the RE Platform message, and opaque keys such
  as `cpu_count` flow to RE.
- `tests/plan34/test_reapi_local_executor_smoke.py` is a repo-owned local REAPI
  smoke. It uses `SLUG_PLAN34_NATIVELINK_BIN` when set, otherwise it discovers a
  sibling checkout's `../nativelink/target/debug/nativelink` binary. `test.py`
  now includes `tests/plan34/` in the normal Python integration entrypoint and
  passes the built Slug binary through `TEST_EXECUTABLE`/`SLUG_BIN`; the smoke
  resolves these executable paths before changing into fixture workspaces. The
  NativeLink smoke runs when a NativeLink binary is available and skips cleanly
  otherwise.
- The smoke starts a local all-in-one NativeLink REAPI service with one worker
  and builds fast in-repo fixtures through `--remote_executor` and
  `--remote_cache`. The shell and platform-exec-properties fixtures
  intentionally omit `--remote-only` to prove the RE-configured default and
  platform-derived executor paths; the C-source and `@rules_cc` fixtures also
  pass `--remote-only` as an explicit strategy check.
- The smoke proves a one-action shell fixture, a one-action platform
  `exec_properties` fixture, a three-action C-source Starlark rule fixture, and
  a real `@rules_cc` `cc_binary` fixture cross REAPI with what-ran
  `executor="Re"` and zero direct-local actions. The `rules_cc` fixture uses
  Bazel-shaped `--action_env=PATH=/usr/bin:/bin` so the NativeLink-local host
  C++ toolchain can find its linker.
- It is repeatable on checkouts with a sibling NativeLink build and is wired
  into the CI test entrypoint. Linux CI now has a repo-owned bootstrap action:
  `.github/actions/setup_plan34_nativelink` clones public NativeLink tag
  `v1.5.2`, verifies commit `6e63ef9a567ac49c77ab258f3af9331336868bb0`,
  builds only the `nativelink` binary with `cargo +stable --profile=smol`, and
  exports `SLUG_PLAN34_NATIVELINK_BIN` before `run_test_py`. The hosted runtime
  of that gate still needs to be observed.
- Legacy explicit
  `CommandExecutorConfig(local_enabled=True, remote_enabled=True)` hybrid
  configs are classified as test/example-only Buck/BXL diagnostic surfaces, not
  Bazel 9 execution-platform compatibility. A Plan 34 guard fails if
  `ExecutionPlatformInfo` or `CommandExecutorConfig` appears in production
  Starlark/BUILD files.

## Accepted Evidence

- `cargo test -p slug_server oss_default_executor_ --lib` proves RE-configured
  OSS defaults are remote-only rather than local-only or hybrid.
- `cargo test -p slug_configured platform_exec_properties_ --lib` proves
  Bazel `platform(exec_properties = {...})` synthesis emits a remote-only
  executor config and filters label-shaped build-setting keys out of
  `re_properties`.
- `cargo test -p slug_server re_config_overlay_projects_reapi_executor_snapshot --lib`
  proves the daemon binds an REAPI executor snapshot into static RE metadata.
- `cargo test -p slug_client_ctx cli_re_config_snapshot_projects_bazel_remote_flags --lib`
  proves Bazel-shaped RE flags reach the daemon startup snapshot.
- `pytest -q tests/plan34/test_legacy_execution_platform_surface.py` proves
  legacy explicit hybrid execution-platform APIs are confined to tests/examples.
- `SLUG_BIN=target/debug/slug python -m pytest tests/plan34/test_reapi_local_executor_smoke.py -q -s`
  proves local NativeLink-backed REAPI execution for repo-owned fixtures with
  `reapi_actions=1` for the no-`--remote-only` shell fixture,
  `reapi_actions=1` for the no-`--remote-only` platform `exec_properties`
  fixture, `reapi_actions=3` for the C-source Starlark rule fixture,
  `reapi_actions=2` for the `@rules_cc` fixture, `direct_local_actions=0`, and
  local command count 0.
- `TEST_EXECUTABLE=target/debug/slug python -m pytest tests/plan34/ -q` proves
  the Plan 34 guard and smoke are reachable through the same Slug-binary
  environment used by `test.py`/CI.
- `.github/actions/setup_plan34_nativelink/action.yml` plus the Linux
  `build-and-test.yml` job wire the Plan 34 smoke to a pinned public NativeLink
  `v1.5.2` source build in CI without secrets or hosted RE endpoints.
- `git ls-remote` and a shallow `git clone --depth=1 --branch v1.5.2
  https://github.com/TraceMachina/nativelink.git ...` both resolve the public
  NativeLink tag to `6e63ef9a567ac49c77ab258f3af9331336868bb0`.
- `cargo +stable build --bin nativelink --profile=smol --locked` succeeds on a
  local NativeLink `v1.5.2` checkout at that commit, validating the CI bootstrap
  build command.
- Python YAML parsing plus `bash -n` over
  `.github/actions/setup_plan34_nativelink/action.yml` validates the local CI
  bootstrap action shape.

The NativeLink smoke is execution-boundary evidence, including a real
`@rules_cc` compile/link proof. It is now wired as a Linux CI gate; the first
hosted run still needs to be recorded as accepted runtime evidence.

## Remaining Gaps

- Observe the first hosted Linux CI run with
  `.github/actions/setup_plan34_nativelink`. If source-building NativeLink makes
  routine CI too slow, keep the same REAPI boundary but switch the bootstrap to a
  faster pinned public artifact/cache path.
- Decide whether the test-owned `--action_env=PATH=/usr/bin:/bin` belongs in
  routine validation or should move behind a hermetic local C++ toolchain setup.
- Prefer NativeLink as the local REAPI service. Use `actiond` only behind that
  REAPI surface if it is needed as the executor backend.
- Keep cache identity and ActionResult replay in Plan 31. Plan 34 only consumes
  cache evidence when it proves the action crossed REAPI.

## Validation Commands

- Focused invariants:
  - `cargo test -p slug_client_ctx cli_re_config_snapshot_projects_bazel_remote_flags --lib`
  - `cargo test -p slug_server re_config_overlay_projects_reapi_executor_snapshot --lib`
  - `cargo test -p slug_server oss_default_executor_ --lib`
- NativeLink/local-REAPI execution proof:
  - `TEST_EXECUTABLE=target/debug/slug python -m pytest tests/plan34/ -q`
  - `SLUG_BIN=target/debug/slug python -m pytest tests/plan34/test_reapi_local_executor_smoke.py -q -s`
  - Set `SLUG_PLAN34_NATIVELINK_BIN=/path/to/nativelink` when no sibling
    `../nativelink/target/debug/nativelink` binary is available.
- CI bootstrap sanity:
  - `git ls-remote --tags https://github.com/TraceMachina/nativelink.git refs/tags/v1.5.2`
  - `git clone --depth=1 --branch v1.5.2 https://github.com/TraceMachina/nativelink.git ...`
  - `cargo +stable build --bin nativelink --profile=smol --locked` from a
    NativeLink `v1.5.2` checkout
  - Python YAML parsing of `.github/workflows/build-and-test.yml`,
    `.github/actions/setup_plan34_nativelink/action.yml`, and
    `.github/actions/run_test_py/action.yml`
  - `bash -n` over the shell blocks in
    `.github/actions/setup_plan34_nativelink/action.yml`

## Next Owner

Observe and record the first hosted Linux CI run of the
`.github/actions/setup_plan34_nativelink` gate. Do not open new flag or
target-language compatibility lanes until the local REAPI path is routine.
