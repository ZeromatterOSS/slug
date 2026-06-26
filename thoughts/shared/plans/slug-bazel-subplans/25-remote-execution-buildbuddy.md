# Plan 25: REAPI transport and remote execution config

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> History before the 2026-06-25 scrubbed-main rewrite lives in
> [25-remote-execution-buildbuddy-history.md](./25-remote-execution-buildbuddy-history.md).

## Goal

Own the transport side of Bazel 9 remote execution compatibility:

- parse Bazel-shaped remote flags;
- restart or refresh daemon RE clients when those flags change;
- bind CAS, Action Cache, Execution, headers, instance name, TLS, and default
  exec properties into Slug's RE client configuration;
- expose enough execution evidence for Plan 34 to prove actions crossed the
  REAPI boundary.

Plan 25 does not own platform selection, per-target `exec_properties` analysis,
or the policy that forbids direct-local execution under a configured REAPI
boundary. Those are Plan 24 and Plan 34.

## Bazel Source Anchors

- Remote endpoint flags and TLS scheme semantics:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/options/RemoteOptions.java:96-155`.
- Remote headers and instance name:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/options/RemoteOptions.java:197-224`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/options/RemoteOptions.java:373-380`.
- Default remote exec properties:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/options/RemoteOptions.java:668-680`.
- Bare `--remote_executor` also supplies the cache endpoint when
  `--remote_cache` is empty:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteModule.java:423-427`.
- Remote execution is enabled only by a non-empty `--remote_executor`:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteModule.java:219-222`.
- Cache-only configuration creates a remote-caching action context with no
  remote executor:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteModule.java:775-838`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteActionContextProvider.java:93-110`.
- Bazel's remote strategy dispatches through REAPI `Execute`:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/RemoteActionContextProvider.java:201-214`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/remote/GrpcRemoteExecutor.java:122-177`.

## Current State

- `CommonBuildConfigurationOptions::cli_re_config_snapshot` projects
  `--remote_executor`, `--remote_cache`, `--remote_*_header`,
  `--remote_instance_name`, `--tls_client_certificate`, and
  `--remote_default_exec_properties` into `DaemonStartupConfig.re_config`.
- `streaming::exec_impl` merges the CLI snapshot before the daemon constraint
  check, so RE config changes participate in daemon restart decisions.
- `apply_re_config_overlay` layers the startup snapshot onto
  `RemoteExecutionStaticMetadata`. A bare executor endpoint fills CAS, engine,
  and action-cache addresses unless a more-specific cache endpoint overrides it.
- Cache-only `--remote_cache` overlays fill CAS and Action Cache addresses but
  leave the engine address empty, so the daemon is not considered
  remote-execution configured and the OSS default executor remains local.
- `get_default_executor_config(..., remote_execution_configured = true, ...)`
  promotes the OSS default executor from direct local to
  `RemoteEnabled(Remote)`.
- Plan 34 now includes a NativeLink/local-REAPI smoke where `--remote_executor`
  is set and `--remote_cache` is omitted. The build executes through REAPI,
  uploads inputs, materializes output, and has zero direct-local actions,
  proving the bare-executor CAS/AC fallback against a real executor service.

This proves configuration reaches the RE client boundary; it does not prove that
every build action executes through REAPI. Plan 34 owns that execution-boundary
proof.

## Accepted Evidence

- `cargo test -p slug_client_ctx cli_re_config_snapshot_projects_bazel_remote_flags --lib`
- `cargo test -p slug_client_ctx cli_re_config_snapshot_keeps_remote_cache_cache_only --lib`
- `cargo test -p slug_server re_config_overlay_projects_reapi_executor_snapshot --lib`
- `cargo test -p slug_server re_config_overlay_keeps_remote_cache_cache_only --lib`
- `cargo test -p slug_server oss_default_executor_ --lib`
- `TEST_EXECUTABLE=$PWD/target/debug/slug python -m pytest tests/plan34/test_reapi_local_executor_smoke.py::test_native_link_bare_remote_executor_supplies_reapi_cache_endpoint -q -s`

## Remaining Gaps

- Plan 34 has a NativeLink/local-REAPI smoke for shell, bare
  `--remote_executor`, platform `exec_properties`, C-source, and `@rules_cc`
  fixtures. Keep transport changes pointed at making that path routine rather
  than adding direct executor shortcuts.
- Keep BuildBuddy hosted execution as supplemental public/open-source evidence
  only. Do not commit credentials, private endpoints, or private workspace names.

## Next Owner

Plan 34 should take the next slice: observe the hosted Linux run of the
NativeLink REAPI gate and keep the local REAPI path routine.
