# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-live-evidence-after-nightly-repair`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one reviewed sanitized proof of BuildBuddy cache-only prime/replay.

## Goal and required design

From the clean scheduling commit, run
`python3 tools/v2_oracle/buildbuddy_cache_gate.py` exactly once. The accepted
nightly repair must remain in the frozen command. Permit only Bazel to consume
ordinary workspace/home RC discovery; no agent or inspection tool may read or
expand home configuration. Review only the compact stdout object, process
status, and empty stderr.

Accept only `PROVED_CACHE_ONLY`: exact selected build plus 43-test completion,
each test executing once in prime and replaying from remote cache, zero
persistent local action-cache hits, identical eligible digest multisets,
eligible prime runners restricted to `local`, `worker`, or `linux-sandbox`,
remote-cache-hit-only eligible replay runners, and successful implicit cleanup
of all private raw state.

## Stops and budget

Return `REPLAN` on any other classification, stderr, retained private path,
schema surprise, unavailable service, target/cache miss, or required code or
configuration repair. Do not make a second attempt, inspect home configuration,
retain raw logs, invoke RBE, or change code/config/CI/BUILD/MODULE/locks/targets/
cycle/core/platform behavior. After success, only the owner plan, canonical
plan, and this manifest may record the sanitized result: at most 100 lines in
three files.
