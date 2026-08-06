# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-cache-evidence-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an offline-tested, secret-safe BuildBuddy cache prime/replay driver and
sanitizer, without an authenticated invocation.

## Goal and required design

Implement the accepted owner-plan contract in the four allowlisted files. The
manifest is exactly one build label plus all 43 green live `rust_test` labels;
its final-newline SHA-256 is
`3a717cb4b0a1f5cab06d336e69d2382861a9c21af9a1502ea20c54b990adf6d5`.
Build a stdlib-only driver that creates two private fresh output bases, runs one
hardened cache-only `bazel test` invocation per phase, parses disposable local
BEP and execution-log JSON sequences, proves the exact digest/runner/cache-hit
and target/test predicates, constructs only the closed sanitized JSON record,
shuts down both private servers, and deletes every raw artifact. Add exhaustive
synthetic/parser and mocked-orchestration tests. The live checkout must include
`//app/slug_core_v2:runtime_test`; only the expected-red CLI fixture is excluded.

## Stops and budget

Return `REPLAN` rather than expose raw data or errors, retain a private artifact,
accept a partial/mixed/local replay, loosen the closed schema, add a dependency,
invent an endpoint/credential path, combine RBE, add CI, run an authenticated
invocation, or change configuration/BUILD/MODULE/locks/evidence/targets/cycle/core/
platform behavior. Only `tools/v2_oracle_lib/buildbuddy_cache.py` (480 lines),
`tools/v2_oracle/buildbuddy_cache_gate.py` (40),
`tests/v2_oracle/buildbuddy_cache_targets.txt` (45),
`tests/v2_oracle/test_buildbuddy_cache_gate.py` (520), and at most 120 owner/
scheduling lines may change: seven files and 1,300 changed lines total.
