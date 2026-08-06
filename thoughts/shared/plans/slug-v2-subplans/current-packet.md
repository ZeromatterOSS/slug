# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-live-evidence-retry`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one reviewed sanitized proof of BuildBuddy cache-only prime/replay.

## Goal and required design

From clean commit `c95c18b4`, run
`python3 tools/v2_oracle/buildbuddy_cache_gate.py` exactly once. Permit only
Bazel to consume ordinary workspace/home RC discovery. Review only the compact
stdout object and process status. Accept only `PROVED_CACHE_ONLY`, exact
43-test/build completion, per-test exact-once execution and replay caching,
zero persistent local action-cache hits, identical eligible digest multisets,
local-only eligible prime runners, remote-cache-hit-only eligible replay
runners, empty stderr, and implicit successful private-root cleanup.

## Stops and budget

Return `REPLAN` on any other classification, stderr, retained raw path, schema
surprise, unavailable service, target/cache miss, or required code/config
change. Do not make a second attempt, inspect home configuration, retain raw
logs, invoke RBE, or change code/config/CI/BUILD/MODULE/locks/targets/cycle/core/
platform behavior. After success, only the owner plan, canonical plan, and this
manifest may record the sanitized result: at most 100 lines in three files.
