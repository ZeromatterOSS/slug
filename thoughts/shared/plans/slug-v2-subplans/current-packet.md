# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-repaired-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one replacement-aware build-cache prime/replay result.

## Goal and required evidence

From the clean scheduling commit, invoke exactly once:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_gate.py
```

Inherit the environment unchanged so only Bazel consumes ordinary/home RC.
Never inspect, print, copy, or persist that RC/token, `HOME`, terminal/BEP/
execution contents, invocation URLs, or BuildBuddy UI data. Review only CLI
status, empty stderr, and one compact normalized JSON record.

Accept only exit zero, schema version one, fixed
`buildbuddy-build-cache-only` mode, and `PROVED_BUILD_CACHE`. The frozen gate
requires both successful builds/materializations, nonempty matching eligible
digest multisets, prime local misses, replay remote-cache hits, zero persistent
hits/errors, anchored shutdown, dual-root cleanup, clean Git, and no `slugd`.

## Stops and budget

Do not retry, inspect artifacts, modify code/config, or reinterpret any failure.
Any nonzero CLI, nonempty stderr, schema surprise, class other than
`PROVED_BUILD_CACHE`, retained state, Git/daemon drift, or cleanup failure is
`REPLAN`. This can prove only one build-label cache vertical.

Afterward only owner/canonical/current docs may record the fixed result, at most
100 changed lines. Structured build-only RBE, the full 43-test expansion, and
the rest of Stage 10 remain required successors.
