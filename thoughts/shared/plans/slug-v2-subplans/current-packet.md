# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-execution-stage-probe-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an offline-reviewed execution-log-only discriminator.

## Goal and required design

Add only library/CLI/tests named
`buildbuddy_build_cache_prime_execution_stage_probe.py` under their existing
directories, capped at 180/30/220 and 430 total lines. Do not edit existing
gate/probes, configuration, targets, fixtures, or docs.

Reuse the exact one-prime command, retained root/phase/output descriptors,
replacement-aware execution reader, anchors, RC-disabled shutdown, dual-root
cleanup, Git, and no-`slugd`. Never read BEP or traverse output. Feed a lazy
stream wrapper into the unchanged spawn parser so an invalid earlier spawn wins
over a malformed later JSON value; expose no execution field, value, digest,
runner, count, error, path, size, or content.

The exact record contains schema version one, fixed mode, classification
`STAGE_RECORDED|SANITIZER_REJECTED`, process `ZERO|NONZERO`, and stage:
`NOT_RECORDED`, `PRECHECK_REJECTED`, `SETUP_REJECTED`, `PROCESS_NONZERO`,
`POST_RUN_ANCHOR_REJECTED`, `EXECUTION_DESCRIPTOR_REJECTED`,
`EXECUTION_STREAM_REJECTED`, `EXECUTION_SPAWN_REJECTED`,
`POST_PARSE_ANCHOR_REJECTED`, or `EXECUTION_READY`. Only precheck/setup/process
pair with nonzero; later recorded stages pair with zero.

## Stops and budget

Offline mocks cover exact argv; empty/valid/replaced execution; descriptor
attacks; invalid spawn and stream order; root/phase/output/read/shutdown swaps;
cleanup/setup/process failures; zero BEP/output readers; schema subclasses;
secret suppression; empty CLI stderr; clean lifecycle. Run focused tests,
compile/caps/scope/diff, and independent review only. No Bazel, home RC,
network, service, or live artifact. Existing-file edit, public expansion,
ambiguous ownership, or second material correction is `REPLAN`. A later packet
owns one transported result; cache/RBE and the 43-test gate remain open.
