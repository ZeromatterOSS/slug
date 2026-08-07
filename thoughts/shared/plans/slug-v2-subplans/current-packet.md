# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-artifact-probe-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a fail-closed metadata-only prime artifact probe.

## Goal and required design

Add only:

- `tools/v2_oracle_lib/buildbuddy_build_cache_artifact_probe.py` (220 lines),
- `tools/v2_oracle/buildbuddy_build_cache_artifact_probe.py` (35), and
- `tests/v2_oracle/test_buildbuddy_build_cache_artifact_probe.py` (260).

The probe reuses `buildbuddy_build_cache.command(...)` exactly for one mocked
prime invocation and its existing label/vector. It precreates private BEP and
execution files, never opens or reads any terminal or evidence contents, and
after the process returns checks only retained-identity no-follow metadata:
`PRIVATE_REGULAR` or `NOT_PRIVATE_REGULAR`. Reuse existing clean-Git/no-`slugd`,
RC-disabled shutdown, retained-root identity, and exact-root cleanup patterns.

The closed record has exactly `schema_version=1`, fixed mode
`buildbuddy-build-cache-prime-artifact-probe`, classification
`PROBE_RECORDED|SANITIZER_REJECTED`, process `ZERO|NONZERO`, and BEP/execution
`PRIVATE_REGULAR|NOT_PRIVATE_REGULAR`. A rejected record uses fixed conservative
values. Never emit an exit code, path, size, time, raw byte, command, nonce,
hostname, RC/auth value, exception, or artifact-derived value. The CLI deeply
normalizes the schema, writes one compact JSON line, and keeps stderr empty.

## Stops and budget

Mocked offline tests cover all eight recorded combinations, wrong/hostile
schema, symlink/hardlink/replacement, proof that artifacts are never read,
exact command reuse, cleanup/shutdown/Git/daemon failures, and secret
suppression. Run only focused tests, Python compilation, caps/diff checks, and
independent review. Do not invoke Bazel, ordinary/home RC, or a remote service;
do not edit the accepted gate, `.bazelrc`, targets, manifest, or other files.

Stop at `REPLAN` if any raw read, exact exit/metadata exposure, cache-gate schema
change, cap breach, or second material correction is required. A separate
packet owns one probe invocation. Structured build-only cache/RBE evidence,
the full 43-test expansion, and the rest of Stage 10 remain required.
