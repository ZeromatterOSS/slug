# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-rbe-vertical-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one offline-reviewed structured one-label RBE driver.

## Goal and required evidence

Add only:

- `tools/v2_oracle_lib/buildbuddy_build_rbe.py` (260)
- `tools/v2_oracle/buildbuddy_build_rbe_gate.py` (20)
- `tests/v2_oracle/test_buildbuddy_build_rbe_gate.py` (300)

Use one mode-0700 `slug-buildbuddy-rbe-*` root and one fresh `rbe/output` base.
Preflight clean Git/no `slugd`, Linux x86_64, `.bazelversion` 9.2.0, and root
`.bazelrc` SHA-256 `e72f4223b6cfffbc96de018849e306ff9cbfdf4ca50248d8fee229a80dc4c805`.
Never read home RC or expand effective options. The exact command after private
paths/nonce substitution is:

```text
bazel --output_base=<private>/rbe/output build --config=buildbuddy-rbe
--@rules_rust//rust/toolchain/channel=nightly --noremote_accept_cached
--noremote_upload_local_results --remote_download_outputs=toplevel
--remote_timeout=900 --jobs=4 --remote_instance_name= --bes_backend=
--bes_results_url= --disk_cache= --build_event_publish_all_actions
--action_env=SLUG_BUILDBUDDY_BUILD_RBE_NONCE=<64hex>
--build_event_json_file=<private>/bep.json
--execution_log_json_file=<private>/execution.json //app/slug_cli_v2:slug
```

The checked-in profile owns executor/cache endpoints, remote-only/no-fallback,
and managed Linux/amd64 properties. Do not duplicate or clear those values.

## Stops and budget

Emit a closed `buildbuddy-build-rbe-only` record with fixed platform/version/RC
fields and one RBE phase. Count every SpawnExec, valid digest, strict remotable/
cache-hit/status/exit semantics, and exact runner buckets `remote_execution`,
`remote_cache_hit`, `local`, `worker`, `linux_sandbox`, and `other`. Success
`PROVED_BUILD_RBE` requires process/BuildFinished/target/output counts one,
nonempty spawns, `remote_execution == count`, and every persistent-cache,
cache-hit, remotable, status, exit, and other-runner error/count zero. Fixed
failures are `CONFIG_DRIFT`, `REMOTE_UNAVAILABLE`, `COMMAND_LINE_FAILURE`,
`TARGET_FAILURE`, `CACHE_HIT_OR_MIXED_EXECUTION`, `EVIDENCE_INCOMPLETE`, and
`SANITIZER_REJECTED`.

Reuse JSON/field/count plus anchored private readers, output check, shutdown,
identity-safe removal, and clean-lifecycle helpers. Add an RBE-local all-spawn
summarizer/classifier; do not reuse filtered cache summaries unchanged. Test
exact argv/preflight, all runners/fields/classes, closed schema, BEP/output,
private attacks, anchors, cleanup suppression, CLI privacy, and no raw leakage.
Run focused plus existing one-label/cache-family regressions, `py_compile`, line/
scope/diff gates, and independent schema/privacy/lifecycle review. No existing
file, Bazel, network, home RC, artifact, service, config, target, or fixture
change/access. A later evidence packet owns one invocation.
