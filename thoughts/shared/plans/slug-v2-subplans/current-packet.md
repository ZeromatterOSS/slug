# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-replay-read-policy-repair`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: one source-reviewed phase-explicit prime/replay command API.

## Goal and required evidence

Change the shared `buildbuddy_build_cache.command` API to require `phase` first,
then retain its existing arguments. Accept only exact strings `prime` and
`replay`; reject unknown strings, non-strings, and subclasses. At the existing
slot immediately after `--config=buildbuddy-cache`, select exactly:

- prime: `--noremote_accept_cached`
- replay: `--remote_accept_cached`

Every other argument and its order stays byte-for-byte unchanged. `run_gate`
passes the loop phase. These six one-prime probes pass literal `prime`:

- `buildbuddy_build_cache_artifact_probe.py`
- `buildbuddy_build_cache_execution_artifact_probe.py`
- `buildbuddy_build_cache_prime_stage_probe.py`
- `buildbuddy_build_cache_prime_bep_stage_probe.py`
- `buildbuddy_build_cache_prime_execution_stage_probe.py`
- `buildbuddy_build_cache_prime_output_semantics_probe.py`

Edit those seven production files and their seven corresponding test files only.
Do not retain a compatibility wrapper or default phase.

## Stops and budget

Pin that identical prime/replay vectors differ only at the read-policy slot;
each contains exactly its own flag and excludes the opposite. Test invalid
phases, paired negative-then-positive order, one shared nonce, distinct output
bases, every unchanged prime probe command, and all existing classification,
lifecycle, cleanup, and schema behavior. Refresh only stale shared-source digest
assertions. Stay within 14 files, 30 production and 70 test changed lines. Run
the seven focused modules, all `test_buildbuddy_build_cache*.py`, `py_compile`,
scope/cap/digest/diff gates, and independent pinned-source review. No CLI,
lifecycle guard, parser, classifier, schema, RC/config, upload/async option,
fixture, docs, unrelated edit, Bazel, network, home RC, artifact, or service.
A later evidence-only packet owns exactly one paired gate invocation.
