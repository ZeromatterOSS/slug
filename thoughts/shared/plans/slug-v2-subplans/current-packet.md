# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-lifecycle-guard-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an offline-reviewed lifecycle guard for one frozen prime child.

## Goal and required evidence

Add only these files:

- `tools/v2_oracle_lib/buildbuddy_build_cache_prime_lifecycle_guard.py` (220)
- `tools/v2_oracle/buildbuddy_build_cache_prime_lifecycle_guard.py` (30)
- `tests/v2_oracle/test_buildbuddy_build_cache_prime_lifecycle_guard.py` (300)

The 550-line stdlib-only guard requires clean Git, no `slugd`, and zero direct
`slug-buildbuddy-prime-*` temporary entries before starting. It invokes exactly
one frozen output-semantics CLI child using literal `python3`, repository cwd,
inherited environment, `shell=False`, and anonymous `TemporaryFile` stdout and
stderr. Bound stdout at 2 KiB; require empty stderr, zero exit, canonical JSON,
and accepted child normalization. Scan only direct temporary names/metadata,
never contents.

Emit only schema version, fixed mode, classification, lifecycle, and a nested
normalized child record. Fixed lifecycle values are `NOT_RECORDED`,
`PRECHECK_REJECTED`, `CHILD_REJECTED`, `ROOT_RESIDUE_REMOVED`,
`ROOT_RESIDUE_REJECTED`, `POSTCHECK_REJECTED`, and `LIFECYCLE_CLEAN`. Only
`LIFECYCLE_CLEAN` exposes a `STAGE_RECORDED` child and exits zero. A single new
root is removed through the existing no-follow helper, followed by a zero-root
recheck, but its stage stays suppressed. Multiple roots, hostile types/swaps,
invalid child records, cleanup failure or false success fail closed. Recheck Git
and `slugd` after every path.

## Stops and budget

Test every lifecycle value, exact child invocation/environment behavior, every
valid stage/process pairing, preexisting-root short-circuit, clean semantic and
ready results, single/multiple residue, hostile type and replacement races,
cleanup failure/false success, child exception/nonzero/stderr/oversize/malformed/
noncanonical output, postcheck drift, schema subclasses, empty CLI stderr, and
secret/path suppression. Run focused plus the 51 related tests, `py_compile`,
line/scope/diff gates, and independent lifecycle review. No Bazel, network,
home RC, live artifact, service, config, or existing-file edit. A later live
packet is separately scheduled only after acceptance.
