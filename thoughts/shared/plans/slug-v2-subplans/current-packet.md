# Current Slug V2 Packet

Packet: `WP-10-m8-bazel-buildbuddy-build-cache-prime-bep-stage-probe-implementation`
Milestone: M8 Bazel developer graph
Owner: `slug-v2-subplans/10-bazel-build-and-bootstrap.md`
Result: an offline-reviewed fixed-enum BEP-only discriminator.

## Goal and required design

Add only library/CLI/tests named
`buildbuddy_build_cache_prime_bep_stage_probe.py` under their existing
directories, capped at 230/35/300 lines and 565 total. Do not edit the accepted
gate, prime-stage probe, configuration, targets, fixtures, or docs.

Reuse the exact one-prime command, retained root/phase/output descriptors,
strict original-inode BEP reader, anchors, RC-disabled shutdown, dual-root
cleanup, Git, and no-`slugd`. Create the command-required execution path but
never open/read it after setup; never traverse output. Preserve streaming
first-failure order without exposing any BEP field, value, label, count, error,
path, hash, size, runner, or content.

The exact record contains schema version one, fixed mode, classification
`STAGE_RECORDED|SANITIZER_REJECTED`, process `ZERO|NONZERO`, and stage:
`NOT_RECORDED`, `PRECHECK_REJECTED`, `SETUP_REJECTED`, `PROCESS_NONZERO`,
`POST_RUN_ANCHOR_REJECTED`, `BEP_DESCRIPTOR_REJECTED`, `BEP_STREAM_REJECTED`,
`BEP_EVENT_REJECTED`, `BEP_TERMINAL_REJECTED`, `BEP_COUNTER_REJECTED`,
`POST_PARSE_ANCHOR_REJECTED`, or `BEP_READY`. Only precheck/setup/process stages
pair with nonzero; later stages pair with zero. `BEP_READY` proves only the
BEP portion of the existing prime phase contract.

## Stops and budget

Offline mocks cover every stage and first-failure order; strict BEP ownership;
all root/phase/output/read/shutdown swaps; cleanup/setup/process failures; zero
execution/output readers; exact argv/shutdown; schema subclasses; secret
suppression; clean lifecycle; frozen gate. Run focused plus related tests,
compile/caps/scope/diff, and independent review only. No Bazel, home RC,
network, service, or live artifact. Ambiguous ownership, public expansion,
existing-file edit, or a second material correction is `REPLAN`. A later packet
owns one transported live probe; cache/RBE and the 43-test gate remain open.
