# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-command-failure-diagnostic-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one reviewed compact cache proof or structured failure diagnosis.

## Goal and required design

From clean implementation commit `b66c0bc3…`, run
`python3 tools/v2_oracle/buildbuddy_cache_gate.py` exactly once. Permit only
Bazel to consume ordinary workspace/home RC discovery. Review only its compact
stdout object, process status, and empty stderr. Accept `PROVED_CACHE_ONLY` as
the cache proof. Accept `COMMAND_LINE_FAILURE` only as a diagnosis when each
phase contains a fixed allowlisted `command_failure_class`; it returns
`REPLAN`, not a cache claim.

## Stops and budget

Return `REPLAN` on any other classification, stderr, retained raw path, schema
surprise, unavailable service, target/cache miss, or required repair. Do not
make a second attempt, inspect home configuration, retain or read raw logs,
invoke RBE, or change code/config/CI/BUILD/MODULE/locks/targets/cycle/core/
platform behavior. Only the owner plan, canonical plan, and this manifest may
record the sanitized result: at most 100 changed lines in three files.
