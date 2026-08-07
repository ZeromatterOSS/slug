# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-full-cache-driver-reconciliation-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one offline-reviewed hardened manifest-aware full-cache driver.

## Goal and required evidence

Rewrite only `tools/v2_oracle_lib/buildbuddy_cache.py` (430 lines),
`tools/v2_oracle/buildbuddy_cache_gate.py` (20), and
`tests/v2_oracle/test_buildbuddy_cache_gate.py` (500), at most 950 total lines.
Do not alter the exact-hash 45-line manifest or any accepted one-label file.
Retain exact `json_sequence`, `_field`, `_boolean`, `_count`, and `_digest`
signatures/semantics used by both one-label drivers, plus legacy `command(...)`
bytes used by `buildbuddy_prime_diagnostic.py`; add `full_command` for this gate.

## Stops and budget

`full_command` keeps the accepted vector order through no-fallback, then exactly
once appends cache-results, runs, sharding, action/test env keys both
`SLUG_BUILDBUDDY_CACHE_GATE_NONCE`, BEP/execution, and 44 labels; forbid opposite
read flags, duplicates, and endpoint/strategy/upload/async reconstruction. Keep
`PROVED_CACHE_ONLY`: both phases prove process success, exactly one BuildFinished,
one production completion/output, and one completion/PASSED summary per each of
43 tests with exact run/cache counts 1 and 0/1; duplicates, missing, or foreign
test summaries fail closed. Remote-test-cache totals are 0 then 43; persistent hits
are zero; eligible counts and digest multisets are equal/nonempty; prime is only
uncached accepted local runners and replay only remote-cache hits.

Replace direct reads/removal and open output with closed canonical normalization,
bounded no-follow/replacement-aware artifacts, executable-output proof, anchored
private roots/output bases, identity-safe shutdown/removal, and pre/post clean
Git/no-`slugd` suppression. Test exact vectors/schema/classes/target counts,
hostile scalar/artifact/path replacements, cleanup failures, CLI privacy, and all
one-label regressions. Run focused/cache-family tests, `py_compile`, scope/caps,
and diff gates plus independent schema/privacy/lifecycle review. No Bazel, network,
home/config/artifact/service/manifest/target/fixture access or change.
