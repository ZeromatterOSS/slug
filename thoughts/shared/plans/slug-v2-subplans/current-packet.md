# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-stage-probe-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: a fixed-enum one-prime parser/lifecycle discriminator.

## Goal and required design

Add only library/CLI/tests named
`buildbuddy_build_cache_prime_stage_probe.py` under their existing directories,
with caps 260/35/320 lines and 615 total. Do not edit the accepted gate,
configuration, targets, manifests, fixtures, or docs.

Reuse the exact build-cache command for one prime plus the accepted retained
root/phase/output descriptors, strict BEP reader, replacement-aware execution
reader, anchor/output helpers, parsers, RC-disabled shutdown, dual-root cleanup,
Git, and no-`slugd`. Catch exceptions only at their owning stage and never emit
any parsed/raw value.

The closed record has exactly schema version one, fixed mode
`buildbuddy-build-cache-prime-stage-probe`, classification
`STAGE_RECORDED|SANITIZER_REJECTED`, process `ZERO|NONZERO`, and stage from:
`NOT_RECORDED`, `PRECHECK_REJECTED`, `SETUP_REJECTED`, `PROCESS_NONZERO`,
`POST_RUN_ANCHOR_REJECTED`, `BEP_DESCRIPTOR_REJECTED`, `BEP_PHASE_REJECTED`,
`EXECUTION_DESCRIPTOR_REJECTED`, `EXECUTION_SPAWN_REJECTED`, `OUTPUT_REJECTED`,
`POST_PARSE_ANCHOR_REJECTED`, `PRIME_SEMANTICS_REJECTED`, or `PRIME_READY`.

`PRIME_SEMANTICS_REJECTED` internally applies only the existing prime success,
materialization, nonempty eligible-spawn, zero-error, and local-runner contract;
it emits no counts. `PRIME_READY` proves only that private prime path, not cache.

## Stops and budget

Offline mocked tests cover exact one-prime command reuse, every stage, retained
BEP/replaced execution, malformed/empty semantics, link/mode/directory and all
swap attacks, RC-disabled shutdown/dual-root cleanup, hostile subclass/schema,
secret suppression, empty CLI stderr, and an unchanged-gate structural check.
Run focused tests, compilation, caps/scope/diff, and independent review only.
No Bazel, home RC, network, remote service, or live artifact.

Any raw/metadata exposure, gate edit, public-stage expansion, second material
correction, or lifecycle ambiguity is `REPLAN`. A separate packet owns one live
probe. Cache/RBE, 43-test expansion, and Stage 10 remain required.
