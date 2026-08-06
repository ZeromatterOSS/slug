# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-evidence-preflight-version-repair`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an offline repair of the Bazelisk version preflight false drift.

## Goal and required design

Change the version preflight argv from invalid
`bazel --ignore_all_rc_files --version` to exact
`bazel --ignore_all_rc_files version`. Parse Bazelisk's captured multiline
stdout and accept exactly one line equal to `Build label: 9.2.0`; reject missing,
duplicate, or different labels. Pin the argv and those cases in synthetic unit
tests. Preserve all other driver bytes and behavior.

## Stops and budget

Only `tools/v2_oracle_lib/buildbuddy_cache.py` and
`tests/v2_oracle/test_buildbuddy_cache_gate.py` may change, at most 35 lines.
Return `REPLAN` rather than change any other preflight, command, sanitizer,
schema, classifier, configuration, manifest, target, or documentation. Do not
run Bazel test/build, use ordinary RC discovery, inspect home configuration,
authenticate, contact a remote service, or rerun the failed live packet.
