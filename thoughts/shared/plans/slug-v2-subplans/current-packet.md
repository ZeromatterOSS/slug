# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-command-failure-diagnostic-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a reviewed secret-safe structured command-failure diagnostic.

## Goal and required design

Extend the cache gate from structured Bazel 9.2
`BuildFinished.failureDetail` only. Admit fixed `COMMAND_LINE_ERROR`, add a
closed per-phase `command_failure_class`, and classify matching exit-2 phases
as `COMMAND_LINE_FAILURE` before target failure. Explicitly allowlist the
accepted command/options, remote configuration, execution configuration, and
build-configuration enum pairs. Unknown, missing, multiple, or malformed
structured data must become `UNKNOWN_COMMAND_LINE_ERROR`; never expose the
free-form message, enum, key, stderr, credential, nonce, or path.

## Stops and budget

Change only `tools/v2_oracle_lib/buildbuddy_cache.py` (90 changed lines) and
`tests/v2_oracle/test_buildbuddy_cache_gate.py` (180 changed lines), at most
270 changed lines total. Offline tests must cover every allowlisted pair and
unknown/malformed cases, prove raw/private fields cannot escape, prove stderr
is not read for diagnosis, and preserve all prior classifications. Do not run
Bazel build/test, discover ordinary/home RCs, inspect home configuration,
contact BuildBuddy, change configuration/manifests/CLI/CI/BUILD/MODULE/locks,
or make a live attempt. A separate reviewed evidence packet owns one later
diagnostic invocation.
