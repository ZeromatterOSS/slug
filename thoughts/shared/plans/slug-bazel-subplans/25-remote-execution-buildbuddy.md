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
- `get_default_executor_config(..., re_configured = true, ...)` promotes the OSS
  default executor from direct local to `RemoteEnabled(Hybrid, Limited)`.

This proves configuration reaches the RE client boundary; it does not prove that
every build action executes through REAPI. Plan 34 owns that execution-boundary
proof.

## Accepted Evidence

- `cargo test -p slug_client_ctx cli_re_config_snapshot_projects_bazel_remote_flags --lib`
- `cargo test -p slug_server re_config_overlay_projects_reapi_executor_snapshot --lib`
- `cargo test -p slug_server oss_default_executor_ --lib`

## Remaining Gaps

- Rebuild a fast repo-owned NativeLink/local-REAPI execution smoke with
  `executor_boundary=reapi`, `direct_local_actions=0`, and useful what-ran
  counts.
- Split `--remote_cache` without `--remote_executor` into a cache-only
  `RemoteEnabledExecutor::Local` path instead of treating cache-only config as
  proof of RE execution.
- Keep BuildBuddy hosted execution as supplemental public/open-source evidence
  only. Do not commit credentials, private endpoints, or private workspace names.

## Next Owner

Plan 34 should take the next slice: rebuild the local REAPI execution proof on
top of this transport config, preferably with NativeLink as the local REAPI
service.
