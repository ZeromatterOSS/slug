# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one reviewed sanitized proof of BuildBuddy cache-only prime/replay.

## Goal and required design

From the clean implementation commit, run
`python3 tools/v2_oracle/buildbuddy_cache_gate.py` exactly once. Allow only the
Bazel child process to consume the ordinary workspace/home RC chain; do not
inspect or expand the home RC. Capture and review only the single compact JSON
object on stdout and its process status. Accept only `PROVED_CACHE_ONLY`, exact
43-test/build success, zero prime/replay persistent local action-cache hits,
zero prime and 43 replay remotely cached tests, identical eligible digest
multisets, local-only eligible prime runners, remote-cache-hit-only eligible
replay runners, empty stderr, and confirmed private-root cleanup.

## Stops and budget

Return `REPLAN` on any non-proof classification, stderr, retained raw path,
schema surprise, unavailable remote service, target/cache miss, or need to
change the frozen driver/configuration. Do not rerun, inspect home configuration,
retain raw logs, add code/config/CI, invoke RBE, or change BUILD/MODULE/locks/
targets/cycle/core/platform behavior. After success, only the owner plan,
canonical plan, and this manifest may record the sanitized result: at most
100 authored lines in three files.
