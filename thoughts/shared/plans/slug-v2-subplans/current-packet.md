# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-vertical-live-evidence`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one sanitized BuildBuddy build-cache prime/replay result.

## Goal and required evidence

From the clean scheduling commit, invoke exactly once:

```sh
python3 tools/v2_oracle/buildbuddy_build_cache_gate.py
```

Inherit the process environment unchanged. The driver invokes Bazel twice with ordinary RC discovery so only Bazel consumes the user's authentication-only home RC.
Never inspect, print, copy, expand, or persist that RC, its token, or any derived authentication value; do not set or inspect `HOME`.

Review only the CLI exit status, empty CLI stderr, and its single compact closed JSON record. Accept only exit zero and `PROVED_BUILD_CACHE` with schema version 1,
fixed `buildbuddy-build-cache-only` mode, and the frozen fixed-key phase summaries. The driver itself requires fresh bases, successful builds/materialization,
a nonempty matching eligible digest multiset, prime local misses, replay remote-cache hits, zero persistent action-cache hits, clean Git/no-`slugd`, RC-disabled shutdown, and exact private-root cleanup.

## Stops and budget

Do not read private terminal/BEP/execution artifacts, invocation URLs, home RC, or BuildBuddy UI data. Do not retry, bisect, modify code/configuration, or reinterpret any non-accepting class.
A nonzero CLI status, nonempty stderr, schema surprise, any class other than `PROVED_BUILD_CACHE`, retained state, Git drift, or daemon/cleanup failure is `REPLAN`.

Afterward only owner/canonical/current scheduling docs may record the fixed result, at most 100 changed lines across three files. This packet can prove only one build-label cache vertical.
Structured build-only RBE proof, expansion of the validated driver to the complete 43-test manifest/invariants, and the rest of Stage 10 remain required successors.
