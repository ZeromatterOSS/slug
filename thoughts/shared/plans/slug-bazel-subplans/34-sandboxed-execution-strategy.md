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
- Bazel 9 strict action env defaults true and provides a fixed Unix
  `PATH=/bin:/usr/bin:/sbin:/usr/sbin` plus `LC_CTYPE=C.UTF-8`, with
  `--action_env` overrides:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/rules/BazelRuleClassProvider.java:73-87,141-208,351-364`.
- Bazel only applies `--action_env` through the default shell environment:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/analysis/actions/SpawnAction.java:575-605,916-944`.

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
  sibling checkout's CI-profile `../nativelink/target/smol/nativelink` binary
  before falling back to `../nativelink/target/debug/nativelink`. `test.py` now
  includes `tests/plan34/` in the normal Python integration entrypoint and
  passes the built Slug binary through `TEST_EXECUTABLE`/`SLUG_BIN`; the smoke
  resolves these executable paths before changing into fixture workspaces. The
  NativeLink smoke runs when a NativeLink binary is available and skips cleanly
  otherwise, except on Linux GitHub Actions where a missing NativeLink binary is
  now a hard failure so the hosted gate cannot pass by silently skipping the
  REAPI proof.
- The smoke starts a local all-in-one NativeLink REAPI service with one worker
  and builds fast in-repo fixtures through `--remote_executor` and
  `--remote_cache`. The shell and platform-exec-properties fixtures
  intentionally omit `--remote-only` to prove the RE-configured default and
  platform-derived executor paths. A bare-`--remote_executor` shell fixture
  intentionally omits `--remote_cache` and proves Plan 25's executor-endpoint
  CAS/AC fallback at the execution boundary. The C-source and `@rules_cc`
  fixtures also pass `--remote-only` as an explicit strategy check.
- The smoke proves a one-action shell fixture, a one-action platform
  `exec_properties` fixture, a three-action C-source Starlark rule fixture, and
  a real `@rules_cc` `cc_binary` fixture cross REAPI with what-ran
  `executor="Re"`, `executor_boundary="reapi"`, and zero direct-local actions.
  It also proves a nested `Args.use_param_file("--cargo_manifest_args=@%s",
  use_always=True)` fixture crosses REAPI with the generated paramfile present
  in the uploaded input tree and no direct-local fallback.
  A two-action cargo-runfiles-shaped fixture now proves a declared directory
  output produced by one RE action can feed a downstream RE action after Slug
  materializes and re-uploads recent RE-produced file inputs when the remote CAS
  reports them missing.
  Each build also runs with `--show-output` and asserts the reported
  `buck-out/...` output path exists, giving explicit output-materialization
  evidence for the REAPI execution path.
  It also reads `log what-uploaded --format json` and asserts one RE upload
  record per executed action with nonzero aggregate uploaded digests/bytes,
  proving the fixture action inputs crossed the CAS/upload side of the REAPI
  boundary.
- A dedicated NativeLink remote Action Cache smoke seeds the local REAPI AC with
  a remote execution, kills the Slug daemon, removes Slug's local persistent AC
  state, and rebuilds through the same NativeLink service. The replay proves
  `CacheQuery` + `Cache` what-ran evidence with `cached=1`, `remote=0`,
  `local=0`, and no direct-local fallback.
  The `rules_cc` fixture no longer needs a test-owned `--action_env=PATH=...`;
  Slug now applies Bazel 9's default shell action env when rules request
  `use_default_shell_env`.
- It is repeatable on checkouts with a sibling NativeLink build and is wired
  into the CI test entrypoint. Linux CI now has a repo-owned bootstrap action:
  `.github/actions/setup_plan34_nativelink` clones public NativeLink tag
  `v1.5.2`, verifies commit `6e63ef9a567ac49c77ab258f3af9331336868bb0`,
  builds only the `nativelink` binary with `cargo +stable --profile=smol`, and
  exports `SLUG_PLAN34_NATIVELINK_BIN` before `run_test_py`. A repo-owned CI
  wiring guard now fails if the Linux job stops running that setup action before
  the Python integration entrypoint. `run_test_py` also exports
  `SLUG_PLAN34_EVIDENCE_JSONL` and uploads a
  `plan34-reapi-evidence-${{ runner.os }}` artifact when the smoke writes
  evidence.
  The hosted runtime of that gate still needs to be observed.
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
  `reapi_actions=1` for the bare-`--remote_executor` shell fixture with no
  explicit `--remote_cache`,
  `reapi_actions=1` for the no-`--remote-only` platform `exec_properties`
  fixture, `reapi_actions=1` for the nested `Args.use_param_file` fixture,
  `reapi_actions=2` for the cargo-runfiles-shaped paramfile fixture,
  `reapi_actions=3` for the C-source Starlark rule fixture,
  `reapi_actions=2` for the `@rules_cc` fixture,
  `executor_boundary="reapi"` on every RE row, `direct_local_actions=0`, and
  local command count 0. Each fixture also has `what-uploaded` evidence with
  `upload_records=reapi_actions`, nonzero uploaded digests/bytes, and a
  materialized `--show-output` path.
- `TEST_EXECUTABLE=$PWD/target/debug/slug python -m pytest tests/plan34/test_reapi_local_executor_smoke.py::test_native_link_remote_action_cache_hit_uses_reapi_without_local_fallback -q -s`
  proves NativeLink REAPI Action Cache lookup/update evidence by forcing a
  second build past Slug's local persistent AC and observing `CacheQuery` plus
  `Cache` with zero remote executions, zero direct-local actions, and a
  materialized `--show-output` path.
- `TEST_EXECUTABLE=target/debug/slug python -m pytest tests/plan34/ -q` proves
  the Plan 34 guard and smoke are reachable through the same Slug-binary
  environment used by `test.py`/CI.
- `env -u SLUG_PLAN34_NATIVELINK_BIN TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug
  python -m pytest -q tests/plan34/ -s --tb=short` proves the local Plan 34
  suite discovers the sibling CI-profile NativeLink `target/smol/nativelink`
  binary by default and still reports zero direct-local actions for the REAPI
  smoke fixtures.
- `python -m pytest -q tests/plan34/test_reapi_local_executor_smoke.py::test_nativelink_binary_env_var_wins
  tests/plan34/test_reapi_local_executor_smoke.py::test_nativelink_binary_discovers_smol_before_debug
  tests/plan34/test_reapi_local_executor_smoke.py::test_nativelink_binary_fails_on_linux_github_actions_without_binary
  tests/plan34/test_reapi_local_executor_smoke.py::test_nativelink_binary_skips_without_binary_outside_linux_github_actions
  --tb=short` proves Linux CI cannot skip the NativeLink REAPI smoke if
  `.github/actions/setup_plan34_nativelink` fails to provision a binary, while
  local and non-Linux hosts can still skip when no local REAPI service is
  available.
- `TEST_EXECUTABLE=$PWD/target/debug/slug python -m pytest tests/core/analysis/test_native_rules.py -q -k 'build_config_defaults or action_env_overrides'`
  proves `ctx.configuration.default_shell_env` contains Bazel-shaped defaults
  and that explicit `--action_env` values override them.
- `TEST_EXECUTABLE=$PWD/target/debug/slug python -m pytest tests/plan34/ -q`
  proves the local NativeLink `rules_cc` REAPI smoke succeeds without a
  test-owned `--action_env=PATH=...`.
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
- `python -m pytest -q tests/plan34/test_ci_gate.py --tb=short` proves the
  Linux workflow step order keeps `.github/actions/setup_plan34_nativelink`
  before `.github/actions/run_test_py`, and that the setup action still builds
  the pinned `target/smol/nativelink` binary and exports
  `SLUG_PLAN34_NATIVELINK_BIN`. It also proves `run_test_py` sets the Plan 34
  evidence JSONL path and uploads the artifact if present.
- `TMPDIR=/var/mnt/dev/slug/.tmp
  SLUG_PLAN34_EVIDENCE_JSONL=/var/mnt/dev/slug/.tmp/plan34-reapi-evidence.jsonl
  TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/plan34/ -s --tb=short` writes 9 evidence records with
  `reapi_actions=12`, `direct_local_actions=0`, `upload_records=12`,
  `cache_query_actions=1`, and `cache_hit_actions=1`.

The NativeLink smoke is execution-boundary evidence, including a real
`@rules_cc` compile/link proof. It is now wired as a Linux CI gate; the first
hosted run still needs to be recorded as accepted runtime evidence.

## Remaining Gaps

- Observe the first hosted Linux CI run with
  `.github/actions/setup_plan34_nativelink` and inspect the uploaded
  `plan34-reapi-evidence-Linux` artifact. If source-building NativeLink makes
  routine CI too slow, keep the same REAPI boundary but switch the bootstrap to a
  faster pinned public artifact/cache path.
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
  - `TMPDIR=/var/mnt/dev/slug/.tmp SLUG_PLAN34_EVIDENCE_JSONL=/var/mnt/dev/slug/.tmp/plan34-reapi-evidence.jsonl TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/plan34/ -s --tb=short`
  - `SLUG_BIN=target/debug/slug python -m pytest tests/plan34/test_reapi_local_executor_smoke.py -q -s`
  - Set `SLUG_PLAN34_NATIVELINK_BIN=/path/to/nativelink` when no sibling
    `../nativelink/target/smol/nativelink` or
    `../nativelink/target/debug/nativelink` binary is available.
- Bazel default shell env:
  - `TEST_EXECUTABLE=$PWD/target/debug/slug python -m pytest tests/core/analysis/test_native_rules.py -q -k 'build_config_defaults or action_env_overrides'`
- CI bootstrap sanity:
  - `python -m pytest -q tests/plan34/test_ci_gate.py --tb=short`
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
`.github/actions/setup_plan34_nativelink` gate, including its uploaded
`plan34-reapi-evidence-Linux` artifact. Do not open new flag or target-language
compatibility lanes until the local REAPI path is routine.
