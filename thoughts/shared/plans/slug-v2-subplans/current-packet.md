# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-complete-command-diagnostic-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one cache proof or exact fixed structured failure diagnosis.

## Goal and required design

From clean implementation commit `fcc754a2…`, run
`python3 tools/v2_oracle/buildbuddy_cache_gate.py` exactly once. Permit only
Bazel to consume ordinary workspace/home RC discovery. Review only compact
stdout, process status, and empty stderr. Accept `PROVED_CACHE_ONLY` as cache
proof. Accept `COMMAND_LINE_FAILURE` only as a diagnosis when prime and replay
carry the same fixed non-`NONE` class; it returns `REPLAN`, not a cache claim.

## Stops and budget

Return `REPLAN` on any other classification, differing fixed classes, stderr,
retained raw path, schema surprise, unavailable service, target/cache miss, or
required repair. Do not make a second attempt, inspect home configuration,
retain/read raw logs, invoke RBE, or change code/config/CI/BUILD/MODULE/locks/
targets/cycle/core/platform behavior. Only owner/canonical/current docs may
record the sanitized result, at most 100 changed lines total.
